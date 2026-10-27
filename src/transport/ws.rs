//! WebSocket adapter: binary frames carrying protobuf `ClientMessage` / `ServerMessage`.
//!
//! Works over HTTP/1.1 (`GET` + `Upgrade`) and HTTP/2 (RFC 8441 extended `CONNECT`).

use std::sync::Arc;

use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{CloseFrame, Message, WebSocket, close_code},
    },
    response::Response,
};
use prost::Message as _;

use super::session_driver::{
    CloseReason, Inbound, SessionTransport, TransportClosed, drive_session,
};
use crate::{
    autocomplete::SuggestService,
    proto::{ClientMessage, ServerMessage},
    shutdown::ShutdownSignal,
};

const TEXT_FRAME_REJECTED: &str = "text frames are not supported; send binary protobuf";
const SHUTDOWN_REASON: &str = "server shutting down; reconnect";
// RFC 6455: the close reason must fit in a control frame (125 bytes minus the 2-byte code).
const MAX_CLOSE_REASON_BYTES: usize = 123;

/// Dependencies of the `/ws` route.
#[derive(Debug, Clone)]
pub struct WsState {
    pub service: Arc<SuggestService>,
    pub shutdown: ShutdownSignal,
    pub max_message_bytes: usize,
}

/// Upgrades the connection and runs a session on it.
pub async fn upgrade(upgrade: WebSocketUpgrade, State(state): State<WsState>) -> Response {
    upgrade
        .max_message_size(state.max_message_bytes)
        .on_upgrade(move |socket| {
            tracing::info!(transport = "ws", "session opened");
            drive_session(WsTransport { socket }, state.service, state.shutdown)
        })
}

struct WsTransport {
    socket: WebSocket,
}

impl SessionTransport for WsTransport {
    async fn recv(&mut self) -> Inbound {
        match self.socket.recv().await {
            Some(Ok(Message::Binary(bytes))) => decode(&bytes),
            Some(Ok(Message::Text(_))) => Inbound::Unsupported(TEXT_FRAME_REJECTED.to_owned()),
            Some(Ok(Message::Ping(_) | Message::Pong(_))) => Inbound::Ignored,
            None | Some(Err(_) | Ok(Message::Close(_))) => Inbound::Closed,
        }
    }

    async fn send(&mut self, message: ServerMessage) -> Result<(), TransportClosed> {
        let frame = Message::Binary(message.encode_to_vec().into());
        self.socket.send(frame).await.map_err(|_| TransportClosed)
    }

    async fn close(mut self, reason: CloseReason) {
        let (code, detail): (u16, String) = match reason {
            CloseReason::Shutdown => (close_code::AWAY, SHUTDOWN_REASON.to_owned()),
            CloseReason::Malformed(detail) => (close_code::INVALID, detail),
            CloseReason::Unsupported(detail) => (close_code::UNSUPPORTED, detail),
        };
        let frame = CloseFrame {
            code,
            reason: truncate_to_boundary(&detail, MAX_CLOSE_REASON_BYTES).into(),
        };
        // The client may already be gone; there is nothing left to do then.
        let _ = self.socket.send(Message::Close(Some(frame))).await;
    }
}

fn decode(bytes: &[u8]) -> Inbound {
    ClientMessage::decode(bytes).map_or_else(
        |error| Inbound::Malformed(format!("binary frame is not a ClientMessage: {error}")),
        Inbound::Message,
    )
}

fn truncate_to_boundary(text: &str, max_bytes: usize) -> &str {
    let mut end: usize = text.len().min(max_bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("short", 10, "short")]
    #[case("abcdef", 3, "abc")]
    #[case("pão", 2, "p")]
    fn truncate_to_boundary_never_splits_a_character(
        #[case] text: &str,
        #[case] max_bytes: usize,
        #[case] expected: &str,
    ) {
        assert_eq!(truncate_to_boundary(text, max_bytes), expected);
    }

    #[test]
    fn decode_garbage_bytes_returns_malformed() {
        let inbound = decode(&[0xff, 0xff, 0xff]);

        assert!(matches!(inbound, Inbound::Malformed(_)));
    }
}
