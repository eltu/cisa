use std::path::PathBuf;

use fst::{BuildError, FormatError};

/// Why an FST index could not be built or loaded.
#[derive(Debug, thiserror::Error)]
pub enum FstEngineError {
    /// The index file could not be read.
    #[error("reading index file {path:?} failed: {source}")]
    ReadIndex {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The bytes are not an FST this version can serve.
    #[error("index is not a valid cisa FST: {0}")]
    InvalidIndex(#[from] FormatError),

    /// Building the index from a corpus failed.
    #[error("building the index failed: {0}")]
    BuildIndex(#[from] BuildError),
}
