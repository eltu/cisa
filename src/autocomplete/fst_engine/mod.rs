//! Adapter: serves suggestions from an FST index built by [`build_index`].

mod error;
mod index_build;
mod key_codec;

use std::path::Path;

use fst::{Completion, Fst, MinCost, TopKParams};

pub use error::FstEngineError;
pub use index_build::{IndexEntry, build_index};

use super::{
    EngineFuture, SuggestError, SuggestQuery, Suggestion, SuggestionEngine,
    normalize::normalize_query,
};

/// Caps the states one top-k search visits, bounding worst-case latency.
pub const DEFAULT_MAX_EXPANSIONS: usize = 4_096;

/// [`SuggestionEngine`] backed by an in-memory FST.
///
/// Searches run inline on the calling task instead of `spawn_blocking`: a
/// bounded top-k is expected to cost microseconds, about the price of the
/// hand-off itself. docs/adr/0006 requires a benchmark to confirm it.
///
/// # Examples
///
/// ```
/// use cisa::autocomplete::{FstEngine, IndexEntry, build_index};
///
/// let bytes = build_index(&[IndexEntry { text: "rust".into(), weight: 1 }]).unwrap();
/// let engine = FstEngine::from_bytes(bytes).unwrap().with_max_expansions(1_000);
/// ```
#[derive(Debug)]
pub struct FstEngine {
    fst: Fst<Vec<u8>, MinCost>,
    max_expansions: usize,
}

impl FstEngine {
    /// Loads the index file at `path` fully into memory.
    pub fn open(path: &Path) -> Result<Self, FstEngineError> {
        let bytes: Vec<u8> = std::fs::read(path).map_err(|source| FstEngineError::ReadIndex {
            path: path.to_owned(),
            source,
        })?;
        Self::from_bytes(bytes)
    }

    /// Serves an index already in memory.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, FstEngineError> {
        Ok(Self {
            fst: Fst::open(bytes)?,
            max_expansions: DEFAULT_MAX_EXPANSIONS,
        })
    }

    /// Overrides [`DEFAULT_MAX_EXPANSIONS`].
    pub fn with_max_expansions(self, max_expansions: usize) -> Self {
        Self {
            max_expansions,
            ..self
        }
    }

    fn top_suggestions(&self, query: &SuggestQuery) -> Result<Vec<Suggestion>, SuggestError> {
        let key_prefix: String = normalize_query(&query.text);
        let params = TopKParams {
            k: query.limit,
            max_expansions: self.max_expansions,
        };
        self.fst
            .top_k(key_prefix.as_bytes(), params)
            .iter()
            .map(to_suggestion)
            .collect()
    }
}

impl SuggestionEngine for FstEngine {
    fn suggest(&self, query: SuggestQuery) -> EngineFuture<'_> {
        let result = self.top_suggestions(&query);
        Box::pin(std::future::ready(result))
    }
}

fn to_suggestion(completion: &Completion<MinCost>) -> Result<Suggestion, SuggestError> {
    // The key is never logged or returned: it holds query text (LGPD).
    let surface: &str = key_codec::decode_surface(&completion.key).map_err(|_| {
        SuggestError::Internal(format!(
            "index key of {} bytes is not UTF-8; expected UTF-8 query text",
            completion.key.len()
        ))
    })?;
    Ok(Suggestion {
        text: surface.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> FstEngine {
        FstEngine::from_bytes(build_index(&[]).unwrap()).unwrap()
    }

    fn query(text: &str, limit: usize) -> SuggestQuery {
        SuggestQuery {
            text: text.to_owned(),
            locale: "pt-BR".to_owned(),
            region: None,
            limit,
        }
    }

    #[tokio::test]
    async fn suggest_mixed_case_prefix_returns_best_completions() {
        // Act
        let suggestions = engine().suggest(query("RUST", 2)).await.unwrap();

        // Assert — STUB (docs/adr/0006): values come from the fst fixture.
        let texts: Vec<&str> = suggestions.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, ["rust async", "rust lang"]);
    }

    #[tokio::test]
    async fn suggest_zero_expansions_returns_empty() {
        // Arrange
        let engine = engine().with_max_expansions(0);

        // Act
        let suggestions = engine.suggest(query("rust", 8)).await.unwrap();

        // Assert
        assert!(suggestions.is_empty());
    }

    #[test]
    fn from_bytes_garbage_returns_invalid_index() {
        let result = FstEngine::from_bytes(b"definitely not a cisa fst file".to_vec());

        assert!(matches!(result, Err(FstEngineError::InvalidIndex(_))));
    }

    #[test]
    fn open_missing_file_returns_read_index() {
        let result = FstEngine::open(Path::new("/nonexistent/cisa/index.fst"));

        assert!(matches!(result, Err(FstEngineError::ReadIndex { .. })));
    }

    #[test]
    fn to_suggestion_invalid_utf8_key_returns_internal() {
        // Arrange
        let completion = Completion {
            key: vec![0xFF],
            output: MinCost::from_weight(1),
        };

        // Act
        let result = to_suggestion(&completion);

        // Assert
        assert!(matches!(result, Err(SuggestError::Internal(_))));
    }
}
