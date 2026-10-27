#![allow(
    clippy::unwrap_used,
    reason = "test helpers: a failure must abort the test"
)]

mod common;

use cisa::{
    autocomplete::MockEngine,
    proto::{
        ClientMessage, ErrorCode, ServerMessage,
        autocomplete_service_client::AutocompleteServiceClient, server_message::Payload,
    },
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Code, Streaming};

struct GrpcSession {
    requests: mpsc::Sender<ClientMessage>,
    responses: Streaming<ServerMessage>,
    server: common::TestServer,
}

async fn open_session() -> GrpcSession {
    let server = common::spawn_server(MockEngine::default()).await;
    let mut client = AutocompleteServiceClient::connect(format!("http://{}", server.addr))
        .await
        .unwrap();
    let (requests, outbound) = mpsc::channel(8);
    let responses = client
        .session(ReceiverStream::new(outbound))
        .await
        .unwrap()
        .into_inner();
    GrpcSession {
        requests,
        responses,
        server,
    }
}

#[tokio::test]
async fn grpc_session_suggest_returns_matching_suggestions() {
    // Arrange
    let mut session = open_session().await;

    // Act
    session
        .requests
        .send(common::suggest_message(1, "receita"))
        .await
        .unwrap();

    // Assert
    let reply = session.responses.message().await.unwrap().unwrap();
    assert_eq!(reply.request_id, 1);
    let Some(Payload::Result(result)) = reply.payload else {
        panic!("expected a result payload");
    };
    assert_eq!(result.suggestions.len(), 2);
}

#[tokio::test]
async fn grpc_session_message_without_payload_returns_invalid_argument() {
    let mut session = open_session().await;

    let message = ClientMessage {
        request_id: 3,
        payload: None,
    };
    session.requests.send(message).await.unwrap();

    let reply = session.responses.message().await.unwrap().unwrap();
    let Some(Payload::Error(error)) = reply.payload else {
        panic!("expected an error payload");
    };
    assert_eq!(error.code(), ErrorCode::InvalidArgument);
}

#[tokio::test]
async fn grpc_session_server_shutdown_ends_stream_with_unavailable() {
    let mut session = open_session().await;

    session.server.shutdown.trigger();

    let status = session.responses.message().await.unwrap_err();
    assert_eq!(status.code(), Code::Unavailable);
}
