//! The only place where protobuf messages and domain types meet.

use crate::{
    autocomplete::{SessionCommand, SessionReply, SuggestError, SuggestParams, Suggestion},
    proto::{
        self, ClientMessage, ErrorCode, ServerMessage, SuggestRequest, SuggestResult,
        client_message::Payload as ClientPayload, server_message::Payload as ServerPayload,
    },
};

const MISSING_PAYLOAD: &str = "client message has no payload; expected `suggest` or `cancel`";
// Internal details stay in server logs; clients only learn the category.
const ENGINE_UNAVAILABLE_MESSAGE: &str = "engine unavailable";
const INTERNAL_MESSAGE: &str = "internal error";

/// Converts a client envelope into a session command, or into the error reply to send back.
pub fn to_command(message: ClientMessage) -> Result<SessionCommand, ServerMessage> {
    let request_id: u64 = message.request_id;
    match message.payload {
        Some(ClientPayload::Suggest(request)) => Ok(SessionCommand::Suggest {
            request_id,
            params: to_params(request),
        }),
        Some(ClientPayload::Cancel(_)) => Ok(SessionCommand::Cancel { request_id }),
        None => {
            let error = SuggestError::InvalidArgument(MISSING_PAYLOAD.to_owned());
            Err(error_message(request_id, &error))
        }
    }
}

/// Converts a session reply into the envelope sent to the client.
pub fn to_server_message(reply: SessionReply) -> ServerMessage {
    match reply.outcome {
        Ok(suggestions) => result_message(reply.request_id, suggestions),
        Err(error) => error_message(reply.request_id, &error),
    }
}

fn to_params(request: SuggestRequest) -> SuggestParams {
    SuggestParams {
        query: request.query,
        locale: request.locale,
        region: request.region,
        limit: request.limit,
    }
}

fn result_message(request_id: u64, suggestions: Vec<Suggestion>) -> ServerMessage {
    let suggestions = suggestions
        .into_iter()
        .map(|s| proto::Suggestion { text: s.text })
        .collect();
    ServerMessage {
        request_id,
        payload: Some(ServerPayload::Result(SuggestResult { suggestions })),
    }
}

fn error_message(request_id: u64, error: &SuggestError) -> ServerMessage {
    let (code, message): (ErrorCode, String) = match error {
        SuggestError::InvalidArgument(detail) => (ErrorCode::InvalidArgument, detail.clone()),
        SuggestError::EngineUnavailable(_) => (
            ErrorCode::EngineUnavailable,
            ENGINE_UNAVAILABLE_MESSAGE.to_owned(),
        ),
        SuggestError::Internal(_) => (ErrorCode::Internal, INTERNAL_MESSAGE.to_owned()),
    };
    ServerMessage {
        request_id,
        payload: Some(ServerPayload::Error(proto::Error {
            code: code.into(),
            message,
        })),
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::proto::Cancel;

    fn error_of(message: ServerMessage) -> proto::Error {
        match message.payload {
            Some(ServerPayload::Error(error)) => error,
            other => panic!("expected an error payload, got {other:?}"),
        }
    }

    fn sample_request() -> SuggestRequest {
        SuggestRequest {
            query: "bolo".into(),
            locale: "pt-BR".into(),
            region: "BR".into(),
            limit: 3,
        }
    }

    #[test]
    fn to_command_suggest_maps_every_field() {
        let payload = Some(ClientPayload::Suggest(sample_request()));

        let command = to_command(ClientMessage {
            request_id: 5,
            payload,
        })
        .unwrap();

        let params = SuggestParams {
            query: "bolo".into(),
            locale: "pt-BR".into(),
            region: "BR".into(),
            limit: 3,
        };
        assert_eq!(
            command,
            SessionCommand::Suggest {
                request_id: 5,
                params
            }
        );
    }

    #[test]
    fn to_command_cancel_returns_cancel_for_same_id() {
        let message = ClientMessage {
            request_id: 9,
            payload: Some(ClientPayload::Cancel(Cancel {})),
        };

        let command = to_command(message).unwrap();

        assert_eq!(command, SessionCommand::Cancel { request_id: 9 });
    }

    #[test]
    fn to_command_missing_payload_returns_invalid_argument_reply() {
        let message = ClientMessage {
            request_id: 3,
            payload: None,
        };

        let reply = to_command(message).unwrap_err();

        assert_eq!(reply.request_id, 3);
        assert_eq!(error_of(reply).code(), ErrorCode::InvalidArgument);
    }

    #[test]
    fn to_server_message_success_returns_suggestions_in_order() {
        let found = vec![
            Suggestion { text: "a".into() },
            Suggestion { text: "b".into() },
        ];
        let reply = SessionReply {
            request_id: 1,
            outcome: Ok(found),
        };

        let message = to_server_message(reply);

        let Some(ServerPayload::Result(result)) = message.payload else {
            panic!("expected a result payload");
        };
        let texts: Vec<&str> = result.suggestions.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, ["a", "b"]);
    }

    #[rstest]
    #[case(SuggestError::InvalidArgument("bad region".into()), ErrorCode::InvalidArgument, "bad region")]
    #[case(SuggestError::EngineUnavailable("gpu oom".into()), ErrorCode::EngineUnavailable, ENGINE_UNAVAILABLE_MESSAGE)]
    #[case(SuggestError::Internal("panic at x.rs".into()), ErrorCode::Internal, INTERNAL_MESSAGE)]
    fn to_server_message_error_maps_code_and_hides_internal_detail(
        #[case] error: SuggestError,
        #[case] expected_code: ErrorCode,
        #[case] expected_message: &str,
    ) {
        let reply = SessionReply {
            request_id: 2,
            outcome: Err(error),
        };

        let error = error_of(to_server_message(reply));

        assert_eq!(error.code(), expected_code);
        assert_eq!(error.message, expected_message);
    }
}
