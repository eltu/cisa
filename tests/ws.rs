#![allow(
    clippy::unwrap_used,
    reason = "test helpers: a failure must abort the test"
)]

mod common;

use std::time::Duration;

use cisa::{
    autocomplete::MockEngine,
    proto::{ClientMessage, ErrorCode, ServerMessage, server_message::Payload},
};
use futures_util::{SinkExt, StreamExt};
use prost::Message as _;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, protocol::frame::coding::CloseCode},
};

type Client = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn connect(engine: MockEngine) -> (Client, common::TestServer) {
    let server = common::spawn_server(engine).await;
    let (client, _) = connect_async(format!("ws://{}/ws", server.addr))
        .await
        .unwrap();
    (client, server)
}

async fn send(client: &mut Client, message: &ClientMessage) {
    let frame = Message::Binary(message.encode_to_vec().into());
    client.send(frame).await.unwrap();
}

async fn next_server_message(client: &mut Client) -> ServerMessage {
    match client.next().await.unwrap().unwrap() {
        Message::Binary(bytes) => ServerMessage::decode(bytes).unwrap(),
        other => panic!("expected a binary frame, got {other:?}"),
    }
}

async fn next_close_code(client: &mut Client) -> CloseCode {
    match client.next().await.unwrap().unwrap() {
        Message::Close(Some(frame)) => frame.code,
        other => panic!("expected a close frame, got {other:?}"),
    }
}

#[tokio::test]
async fn ws_suggest_returns_matching_suggestions() {
    // Arrange
    let (mut client, _server) = connect(MockEngine::default()).await;

    // Act
    send(&mut client, &common::suggest_message(1, "previsão")).await;

    // Assert
    let reply = next_server_message(&mut client).await;
    assert_eq!(reply.request_id, 1);
    let Some(Payload::Result(result)) = reply.payload else {
        panic!("expected a result payload");
    };
    assert_eq!(result.suggestions.len(), 2);
}

#[tokio::test]
async fn ws_new_request_supersedes_previous_one() {
    // Arrange: latency keeps request 1 in flight while request 2 arrives.
    let (mut client, _server) = connect(MockEngine::with_latency(Duration::from_millis(100))).await;

    // Act
    send(&mut client, &common::suggest_message(1, "rust")).await;
    send(&mut client, &common::suggest_message(2, "rust a")).await;

    // Assert
    assert_eq!(next_server_message(&mut client).await.request_id, 2);
}

#[tokio::test]
async fn ws_message_without_payload_returns_invalid_argument() {
    let (mut client, _server) = connect(MockEngine::default()).await;

    send(
        &mut client,
        &ClientMessage {
            request_id: 4,
            payload: None,
        },
    )
    .await;

    let Some(Payload::Error(error)) = next_server_message(&mut client).await.payload else {
        panic!("expected an error payload");
    };
    assert_eq!(error.code(), ErrorCode::InvalidArgument);
}

#[tokio::test]
async fn ws_text_frame_closes_with_unsupported_code() {
    let (mut client, _server) = connect(MockEngine::default()).await;

    client.send(Message::Text("hello".into())).await.unwrap();

    assert_eq!(next_close_code(&mut client).await, CloseCode::Unsupported);
}

#[tokio::test]
async fn ws_undecodable_binary_closes_with_invalid_code() {
    let (mut client, _server) = connect(MockEngine::default()).await;

    client
        .send(Message::Binary(vec![0xff, 0xff, 0xff].into()))
        .await
        .unwrap();

    assert_eq!(next_close_code(&mut client).await, CloseCode::Invalid);
}

#[tokio::test]
async fn ws_server_shutdown_closes_with_away_code() {
    let (mut client, server) = connect(MockEngine::default()).await;

    server.shutdown.trigger();

    assert_eq!(next_close_code(&mut client).await, CloseCode::Away);
}
