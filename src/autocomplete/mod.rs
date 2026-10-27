//! Search autocomplete domain: validation, the suggestion engine port and session rules.
//!
//! Nothing in this module knows about gRPC, WebSocket or protobuf.

mod error;
mod mock_engine;
mod service;
mod session;

use std::{future::Future, pin::Pin};

pub use error::SuggestError;
pub use mock_engine::MockEngine;
pub use service::{SuggestLimits, SuggestService};
pub use session::{SessionCommand, SessionReply, SuggestSession};

/// Raw suggest parameters as received from a client, before validation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SuggestParams {
    pub query: String,
    pub locale: String,
    pub region: String,
    pub limit: u32,
}

/// Validated and normalized input handed to a [`SuggestionEngine`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestQuery {
    pub text: String,
    pub locale: String,
    /// Uppercase ISO 3166-1 alpha-2 code, or `None` when the client sent no region.
    pub region: Option<String>,
    pub limit: usize,
}

/// One suggested search query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub text: String,
}

/// Future returned by [`SuggestionEngine::suggest`].
pub type EngineFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<Suggestion>, SuggestError>> + Send + 'a>>;

/// Port for the backend that produces suggestions (mock, index, AI model...).
///
/// # Examples
///
/// ```
/// use cisa::autocomplete::{EngineFuture, SuggestQuery, SuggestionEngine};
///
/// #[derive(Debug)]
/// struct EmptyEngine;
///
/// impl SuggestionEngine for EmptyEngine {
///     fn suggest(&self, _query: SuggestQuery) -> EngineFuture<'_> {
///         Box::pin(async { Ok(Vec::new()) })
///     }
/// }
/// ```
pub trait SuggestionEngine: std::fmt::Debug + Send + Sync + 'static {
    /// Returns at most `query.limit` suggestions, most relevant first.
    fn suggest(&self, query: SuggestQuery) -> EngineFuture<'_>;
}
