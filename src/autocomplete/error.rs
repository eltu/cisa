/// Why a suggest request could not be served.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SuggestError {
    /// The client sent a request that will never succeed as is.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// The engine is overloaded or down; the client may retry later.
    #[error("engine unavailable: {0}")]
    EngineUnavailable(String),

    /// A server bug. The detail is for logs only.
    #[error("internal error: {0}")]
    Internal(String),
}
