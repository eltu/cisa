//! Delivery adapters. One HTTP/1.1 + HTTP/2 router serves every protocol on a single port.

mod grpc;
mod proto;
mod session_driver;
mod ws;

use std::sync::Arc;

use axum::{
    Router,
    routing::{any, get},
};
use tonic::service::Routes;

use crate::{
    autocomplete::SuggestService, proto::autocomplete_service_server::AutocompleteServiceServer,
    shutdown::ShutdownSignal,
};

const HEALTH_PATH: &str = "/health";
const WS_PATH: &str = "/ws";
const HEALTH_BODY: &str = "ok";

/// Builds the single router serving `/health`, `/ws` and the gRPC service.
///
/// # Examples
///
/// ```no_run
/// use std::sync::Arc;
/// use cisa::{autocomplete::*, shutdown, transport};
///
/// # async fn run() -> std::io::Result<()> {
/// let service = Arc::new(SuggestService::new(Arc::new(MockEngine::default()), SuggestLimits::default()));
/// let (_trigger, signal) = shutdown::channel();
/// let router = transport::build_router(service, signal, 4096);
/// let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
/// axum::serve(listener, router).await
/// # }
/// ```
pub fn build_router(
    service: Arc<SuggestService>,
    shutdown: ShutdownSignal,
    max_message_bytes: usize,
) -> Router {
    let grpc = AutocompleteServiceServer::new(grpc::GrpcAutocomplete::new(
        Arc::clone(&service),
        shutdown.clone(),
    ))
    .max_decoding_message_size(max_message_bytes);
    let ws_state = ws::WsState {
        service,
        shutdown,
        max_message_bytes,
    };
    Router::new()
        .route(HEALTH_PATH, get(health))
        // `any`: HTTP/1.1 upgrades use GET, HTTP/2 WebSockets use CONNECT.
        .route(WS_PATH, any(ws::upgrade))
        .with_state(ws_state)
        .merge(Routes::new(grpc).into_axum_router())
}

async fn health() -> &'static str {
    HEALTH_BODY
}
