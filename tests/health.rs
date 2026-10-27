#![allow(
    clippy::unwrap_used,
    reason = "test helpers: a failure must abort the test"
)]

mod common;

use cisa::autocomplete::MockEngine;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

const HEALTH_REQUEST: &[u8] =
    b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";

#[tokio::test]
async fn health_get_returns_ok_over_http1() {
    // Arrange
    let server = common::spawn_server(MockEngine::default()).await;
    let mut stream = TcpStream::connect(server.addr).await.unwrap();

    // Act
    stream.write_all(HEALTH_REQUEST).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();

    // Assert
    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
    assert!(response.ends_with("\r\n\r\nok"), "{response}");
}
