use std::sync::Arc;

use cisa::{
    autocomplete::{MockEngine, SuggestService},
    config::Config,
    shutdown::{self, ShutdownTrigger},
    transport,
};
use tracing_subscriber::EnvFilter;

const DEFAULT_LOG_FILTER: &str = "cisa=info";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();
    let config = Config::from_env()?;
    // TODO: replace MockEngine with the real suggestion engine.
    let engine = Arc::new(MockEngine::default());
    let service = Arc::new(SuggestService::new(engine, config.limits.clone()));
    let (trigger, signal) = shutdown::channel();
    let router = transport::build_router(service, signal, config.max_message_bytes());

    let listener = tokio::net::TcpListener::bind(config.addr).await?;
    tracing::info!(addr = %config.addr, "cisa listening (HTTP/1.1 + HTTP/2: /health, /ws, gRPC)");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_on_os_signal(trigger))
        .await?;
    Ok(())
}

fn init_tracing() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

async fn shutdown_on_os_signal(trigger: ShutdownTrigger) {
    tokio::select! {
        () = ctrl_c() => {},
        () = terminate() => {},
    }
    tracing::info!("shutting down; closing open sessions");
    trigger.trigger();
}

async fn ctrl_c() {
    tokio::signal::ctrl_c()
        .await
        .expect("installing the Ctrl+C handler only fails if called twice");
}

#[cfg(unix)]
async fn terminate() {
    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("installing the SIGTERM handler only fails outside a tokio runtime")
        .recv()
        .await;
}

#[cfg(not(unix))]
async fn terminate() {
    // no-op: SIGTERM does not exist outside unix; only Ctrl+C stops the server.
    std::future::pending::<()>().await;
}
