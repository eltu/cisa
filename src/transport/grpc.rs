//! gRPC adapter: `AutocompleteService.Session` over HTTP/2.

use std::sync::Arc;

use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};

use super::session_driver::{
    CloseReason, Inbound, OUTBOX_CAPACITY, SessionTransport, TransportClosed, drive_session,
};
use crate::{
    autocomplete::SuggestService,
    proto::{ClientMessage, ServerMessage, autocomplete_service_server::AutocompleteService},
    shutdown::ShutdownSignal,
};

const SHUTDOWN_STATUS: &str = "server shutting down; reconnect";

/// Implements the generated gRPC service on top of the shared session loop.
#[derive(Debug, Clone)]
pub struct GrpcAutocomplete {
    service: Arc<SuggestService>,
    shutdown: ShutdownSignal,
}

impl GrpcAutocomplete {
    /// Creates the adapter; sessions end when `shutdown` fires.
    pub fn new(service: Arc<SuggestService>, shutdown: ShutdownSignal) -> Self {
        Self { service, shutdown }
    }
}

#[tonic::async_trait]
impl AutocompleteService for GrpcAutocomplete {
    type SessionStream = ReceiverStream<Result<ServerMessage, Status>>;

    async fn session(
        &self,
        request: Request<Streaming<ClientMessage>>,
    ) -> Result<Response<Self::SessionStream>, Status> {
        let (outbound, stream) = mpsc::channel(OUTBOX_CAPACITY);
        let transport = GrpcTransport {
            inbound: request.into_inner(),
            outbound,
        };
        tracing::info!(transport = "grpc", "session opened");
        tokio::spawn(drive_session(
            transport,
            Arc::clone(&self.service),
            self.shutdown.clone(),
        ));
        Ok(Response::new(ReceiverStream::new(stream)))
    }
}

struct GrpcTransport {
    inbound: Streaming<ClientMessage>,
    outbound: mpsc::Sender<Result<ServerMessage, Status>>,
}

impl SessionTransport for GrpcTransport {
    async fn recv(&mut self) -> Inbound {
        match self.inbound.message().await {
            Ok(Some(message)) => Inbound::Message(message),
            Ok(None) => Inbound::Closed,
            Err(status) => Inbound::Malformed(status.message().to_owned()),
        }
    }

    async fn send(&mut self, message: ServerMessage) -> Result<(), TransportClosed> {
        self.outbound
            .send(Ok(message))
            .await
            .map_err(|_| TransportClosed)
    }

    async fn close(self, reason: CloseReason) {
        let status = match reason {
            CloseReason::Shutdown => Status::unavailable(SHUTDOWN_STATUS),
            CloseReason::Malformed(detail) | CloseReason::Unsupported(detail) => {
                Status::invalid_argument(detail)
            }
        };
        // The client may already be gone; there is nothing left to do then.
        let _ = self.outbound.send(Err(status)).await;
    }
}
