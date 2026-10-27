//! Transport-agnostic session loop shared by the gRPC and WebSocket adapters.

use std::{future::Future, sync::Arc};

use tokio::sync::mpsc;

use super::proto::{to_command, to_server_message};
use crate::{
    autocomplete::{SessionReply, SuggestService, SuggestSession},
    proto::{ClientMessage, ServerMessage},
    shutdown::ShutdownSignal,
};

/// Bounded so a client that stops reading cannot grow server memory.
pub const OUTBOX_CAPACITY: usize = 32;

/// What a transport produced when asked for the next inbound message.
#[derive(Debug)]
pub enum Inbound {
    Message(ClientMessage),
    /// Transport-level traffic with no session meaning (e.g. WebSocket ping).
    Ignored,
    /// The client went away.
    Closed,
    /// The bytes could not be decoded as a `ClientMessage`.
    Malformed(String),
    /// A frame kind the protocol does not accept (e.g. WebSocket text).
    Unsupported(String),
}

/// Why the server ends a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseReason {
    Shutdown,
    Malformed(String),
    Unsupported(String),
}

/// Returned by [`SessionTransport::send`] when the client is gone.
#[derive(Debug)]
pub struct TransportClosed;

/// The minimal I/O a protocol adapter provides to run a session.
pub trait SessionTransport: Send {
    fn recv(&mut self) -> impl Future<Output = Inbound> + Send;
    fn send(
        &mut self,
        message: ServerMessage,
    ) -> impl Future<Output = Result<(), TransportClosed>> + Send;
    fn close(self, reason: CloseReason) -> impl Future<Output = ()> + Send;
}

enum Step {
    Continue,
    Send(ServerMessage),
    Stop,
    Close(CloseReason),
}

enum Flow {
    Continue,
    End(Option<CloseReason>),
}

/// Runs one client session until the client leaves, misbehaves or the server shuts down.
pub async fn drive_session<T: SessionTransport>(
    mut transport: T,
    service: Arc<SuggestService>,
    mut shutdown: ShutdownSignal,
) {
    let (outbox, mut replies) = mpsc::channel(OUTBOX_CAPACITY);
    let mut session = SuggestSession::new(service, outbox);
    let close_reason = run_loop(&mut transport, &mut session, &mut replies, &mut shutdown).await;
    // Abort in-flight work before the (possibly slow) close handshake.
    drop(session);
    if let Some(reason) = close_reason {
        transport.close(reason).await;
    }
}

async fn run_loop<T: SessionTransport>(
    transport: &mut T,
    session: &mut SuggestSession,
    replies: &mut mpsc::Receiver<SessionReply>,
    shutdown: &mut ShutdownSignal,
) -> Option<CloseReason> {
    loop {
        let step: Step = tokio::select! {
            inbound = transport.recv() => on_inbound(session, inbound),
            Some(reply) = replies.recv() => Step::Send(to_server_message(reply)),
            () = shutdown.wait() => Step::Close(CloseReason::Shutdown),
        };
        if let Flow::End(reason) = apply(transport, step).await {
            return reason;
        }
    }
}

async fn apply<T: SessionTransport>(transport: &mut T, step: Step) -> Flow {
    match step {
        Step::Continue => Flow::Continue,
        Step::Send(message) => match transport.send(message).await {
            Ok(()) => Flow::Continue,
            Err(TransportClosed) => Flow::End(None),
        },
        Step::Stop => Flow::End(None),
        Step::Close(reason) => Flow::End(Some(reason)),
    }
}

fn on_inbound(session: &mut SuggestSession, inbound: Inbound) -> Step {
    match inbound {
        Inbound::Message(message) => match to_command(message) {
            Ok(command) => {
                session.handle(command);
                Step::Continue
            }
            Err(reply) => Step::Send(reply),
        },
        Inbound::Ignored => Step::Continue,
        Inbound::Closed => Step::Stop,
        Inbound::Malformed(detail) => Step::Close(CloseReason::Malformed(detail)),
        Inbound::Unsupported(detail) => Step::Close(CloseReason::Unsupported(detail)),
    }
}
