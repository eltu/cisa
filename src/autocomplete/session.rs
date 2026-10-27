use std::{sync::Arc, time::Instant};

use tokio::{sync::mpsc, task::AbortHandle};

use super::{SuggestError, SuggestParams, SuggestService, Suggestion};

/// A client instruction received on a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionCommand {
    Suggest {
        request_id: u64,
        params: SuggestParams,
    },
    Cancel {
        request_id: u64,
    },
}

/// Outcome of one suggest request, delivered on the session outbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionReply {
    pub request_id: u64,
    pub outcome: Result<Vec<Suggestion>, SuggestError>,
}

#[derive(Debug)]
struct InFlight {
    request_id: u64,
    task: AbortHandle,
}

/// Per-connection request state shared by every transport.
///
/// At most one request runs at a time: a new suggest request supersedes the one in
/// flight, because in search autocomplete only the latest keystroke matters.
/// Cancelled and superseded requests get no reply.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use cisa::autocomplete::*;
///
/// # tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
/// let service = Arc::new(SuggestService::new(Arc::new(MockEngine::default()), SuggestLimits::default()));
/// let (outbox, mut replies) = tokio::sync::mpsc::channel(8);
/// let mut session = SuggestSession::new(service, outbox);
///
/// let params = SuggestParams { query: "rust".into(), ..Default::default() };
/// session.handle(SessionCommand::Suggest { request_id: 1, params });
/// assert_eq!(replies.recv().await.unwrap().request_id, 1);
/// # });
/// ```
#[derive(Debug)]
pub struct SuggestSession {
    service: Arc<SuggestService>,
    outbox: mpsc::Sender<SessionReply>,
    in_flight: Option<InFlight>,
}

impl SuggestSession {
    /// Creates a session whose replies are sent to `outbox`.
    pub fn new(service: Arc<SuggestService>, outbox: mpsc::Sender<SessionReply>) -> Self {
        Self {
            service,
            outbox,
            in_flight: None,
        }
    }

    /// Applies one client command. Must be called inside a tokio runtime.
    pub fn handle(&mut self, command: SessionCommand) {
        match command {
            SessionCommand::Suggest { request_id, params } => self.start(request_id, params),
            SessionCommand::Cancel { request_id } => self.cancel(request_id),
        }
    }

    fn start(&mut self, request_id: u64, params: SuggestParams) {
        if self.is_running(request_id) {
            self.reject_duplicate(request_id);
            return;
        }
        self.abort_in_flight();
        let task = tokio::spawn(run_request(
            Arc::clone(&self.service),
            self.outbox.clone(),
            request_id,
            params,
        ));
        self.in_flight = Some(InFlight {
            request_id,
            task: task.abort_handle(),
        });
    }

    // Unknown or already finished ids are ignored: the client and server race by design.
    fn cancel(&mut self, request_id: u64) {
        let matches: bool = self
            .in_flight
            .as_ref()
            .is_some_and(|f| f.request_id == request_id);
        if matches {
            self.abort_in_flight();
        }
    }

    fn is_running(&self, request_id: u64) -> bool {
        self.in_flight
            .as_ref()
            .is_some_and(|f| f.request_id == request_id && !f.task.is_finished())
    }

    fn abort_in_flight(&mut self) {
        if let Some(in_flight) = self.in_flight.take() {
            in_flight.task.abort();
        }
    }

    // try_send: the transport loop that drains the outbox is the caller, so awaiting
    // here could deadlock. A full outbox means the client is not reading; dropping is fine.
    fn reject_duplicate(&self, request_id: u64) {
        let error = SuggestError::InvalidArgument(format!(
            "request_id {request_id} is already in flight; use a new id"
        ));
        let reply = SessionReply {
            request_id,
            outcome: Err(error),
        };
        if self.outbox.try_send(reply).is_err() {
            tracing::debug!(request_id, "outbox full; duplicate-id error dropped");
        }
    }
}

impl Drop for SuggestSession {
    fn drop(&mut self) {
        self.abort_in_flight();
    }
}

async fn run_request(
    service: Arc<SuggestService>,
    outbox: mpsc::Sender<SessionReply>,
    request_id: u64,
    params: SuggestParams,
) {
    let started = Instant::now();
    let query_bytes: usize = params.query.len();
    let outcome = service.suggest(params).await;
    log_outcome(request_id, query_bytes, started, &outcome);
    // Fails only when the transport is gone; nobody is left to notify.
    let _ = outbox
        .send(SessionReply {
            request_id,
            outcome,
        })
        .await;
}

// Logs sizes and timings only: search queries are personal data and are never logged.
fn log_outcome(
    request_id: u64,
    query_bytes: usize,
    started: Instant,
    outcome: &Result<Vec<Suggestion>, SuggestError>,
) {
    let elapsed_us = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    match outcome {
        Ok(found) => {
            let suggestions: usize = found.len();
            tracing::debug!(
                request_id,
                query_bytes,
                elapsed_us,
                suggestions,
                "suggest ok"
            );
        }
        Err(error) => tracing::warn!(request_id, query_bytes, elapsed_us, %error, "suggest failed"),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::sync::mpsc::error::TryRecvError;

    use super::*;
    use crate::autocomplete::{MockEngine, SuggestLimits};

    const ENGINE_LATENCY: Duration = Duration::from_millis(100);

    fn session() -> (SuggestSession, mpsc::Receiver<SessionReply>) {
        let engine = Arc::new(MockEngine::with_latency(ENGINE_LATENCY));
        let service = Arc::new(SuggestService::new(engine, SuggestLimits::default()));
        let (outbox, replies) = mpsc::channel(8);
        (SuggestSession::new(service, outbox), replies)
    }

    fn suggest(request_id: u64, query: &str) -> SessionCommand {
        SessionCommand::Suggest {
            request_id,
            params: SuggestParams {
                query: query.to_owned(),
                ..SuggestParams::default()
            },
        }
    }

    #[tokio::test(start_paused = true)]
    async fn handle_suggest_sends_reply_with_same_request_id() {
        let (mut session, mut replies) = session();

        session.handle(suggest(7, "rust"));

        let reply = replies.recv().await.unwrap();
        assert_eq!(reply.request_id, 7);
        assert_eq!(reply.outcome.unwrap().len(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn handle_new_suggest_supersedes_request_in_flight() {
        // Arrange
        let (mut session, mut replies) = session();

        // Act
        session.handle(suggest(1, "rust"));
        session.handle(suggest(2, "rust a"));

        // Assert
        assert_eq!(replies.recv().await.unwrap().request_id, 2);
        assert_eq!(replies.try_recv().unwrap_err(), TryRecvError::Empty);
    }

    #[tokio::test(start_paused = true)]
    async fn handle_cancel_in_flight_request_sends_no_reply() {
        // Arrange
        let (mut session, mut replies) = session();
        session.handle(suggest(1, "rust"));

        // Act
        session.handle(SessionCommand::Cancel { request_id: 1 });
        tokio::time::sleep(ENGINE_LATENCY * 2).await; // paused clock: advances instantly

        // Assert
        assert_eq!(replies.try_recv().unwrap_err(), TryRecvError::Empty);
    }

    #[tokio::test(start_paused = true)]
    async fn handle_cancel_unknown_id_keeps_request_in_flight() {
        let (mut session, mut replies) = session();
        session.handle(suggest(1, "rust"));

        session.handle(SessionCommand::Cancel { request_id: 99 });

        assert_eq!(replies.recv().await.unwrap().request_id, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn handle_duplicate_in_flight_id_returns_invalid_argument() {
        // Arrange
        let (mut session, mut replies) = session();
        session.handle(suggest(1, "rust"));

        // Act
        session.handle(suggest(1, "rust a"));

        // Assert
        let rejected = replies.recv().await.unwrap();
        assert!(matches!(
            rejected.outcome,
            Err(SuggestError::InvalidArgument(_))
        ));
        assert!(replies.recv().await.unwrap().outcome.is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn drop_session_aborts_request_in_flight() {
        let (mut session, mut replies) = session();
        session.handle(suggest(1, "rust"));

        drop(session);

        assert!(replies.recv().await.is_none());
    }
}
