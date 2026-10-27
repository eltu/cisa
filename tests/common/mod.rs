#![allow(
    dead_code,
    reason = "each integration test binary uses a different subset of these helpers"
)]

use std::{net::SocketAddr, sync::Arc};

use cisa::{
    autocomplete::{MockEngine, SuggestLimits, SuggestService},
    proto::{ClientMessage, SuggestRequest, client_message::Payload},
    shutdown::{self, ShutdownTrigger},
    transport,
};

const MAX_MESSAGE_BYTES: usize = 4096;

/// A server running on an ephemeral port. Dropping it does not stop the server task;
/// the test runtime ends it.
pub struct TestServer {
    pub addr: SocketAddr,
    pub shutdown: ShutdownTrigger,
}

pub async fn spawn_server(engine: MockEngine) -> TestServer {
    let service = Arc::new(SuggestService::new(
        Arc::new(engine),
        SuggestLimits::default(),
    ));
    let (trigger, signal) = shutdown::channel();
    let router = transport::build_router(service, signal, MAX_MESSAGE_BYTES);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    TestServer {
        addr,
        shutdown: trigger,
    }
}

pub fn suggest_message(request_id: u64, query: &str) -> ClientMessage {
    let request = SuggestRequest {
        query: query.to_owned(),
        ..SuggestRequest::default()
    };
    ClientMessage {
        request_id,
        payload: Some(Payload::Suggest(request)),
    }
}
