use std::time::Duration;

use super::{EngineFuture, SuggestQuery, Suggestion, SuggestionEngine};

// Fixture data mirrors the default pt-BR locale so the mock behaves like real traffic.
const MOCK_QUERIES: &[&str] = &[
    "como fazer bolo de cenoura",
    "como fazer bolo de chocolate",
    "como fazer bolo de fubá",
    "como fazer pão caseiro",
    "como fazer panqueca",
    "previsão do tempo",
    "previsão do tempo amanhã",
    "preço do dólar hoje",
    "receita de lasanha",
    "receita de strogonoff",
    "resultado da mega-sena",
    "rust programming language",
    "rust async tutorial",
];

/// Deterministic engine for development and tests: prefix-matches a fixed query list.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use cisa::autocomplete::MockEngine;
///
/// let instant = MockEngine::default();
/// let slow = MockEngine::with_latency(Duration::from_millis(50));
/// ```
#[derive(Debug, Default)]
pub struct MockEngine {
    latency: Duration,
}

impl MockEngine {
    /// Simulates inference time, so cancellation and supersede paths can be exercised.
    pub fn with_latency(latency: Duration) -> Self {
        Self { latency }
    }
}

impl SuggestionEngine for MockEngine {
    fn suggest(&self, query: SuggestQuery) -> EngineFuture<'_> {
        Box::pin(async move {
            if !self.latency.is_zero() {
                tokio::time::sleep(self.latency).await;
            }
            Ok(matching_queries(&query.text, query.limit))
        })
    }
}

fn matching_queries(text: &str, limit: usize) -> Vec<Suggestion> {
    let needle: String = text.to_lowercase();
    MOCK_QUERIES
        .iter()
        .filter(|candidate| candidate.starts_with(&needle))
        .take(limit)
        .map(|candidate| Suggestion {
            text: (*candidate).to_owned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(text: &str, limit: usize) -> SuggestQuery {
        SuggestQuery {
            text: text.to_owned(),
            locale: "pt-BR".to_owned(),
            region: None,
            limit,
        }
    }

    #[tokio::test]
    async fn suggest_mixed_case_prefix_returns_matches_in_order() {
        let engine = MockEngine::default();

        let suggestions = engine.suggest(query("PREVISÃO", 8)).await.unwrap();

        let texts: Vec<&str> = suggestions.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, ["previsão do tempo", "previsão do tempo amanhã"]);
    }

    #[tokio::test]
    async fn suggest_more_matches_than_limit_returns_only_limit() {
        let engine = MockEngine::default();

        let suggestions = engine.suggest(query("como fazer", 2)).await.unwrap();

        assert_eq!(suggestions.len(), 2);
    }

    #[tokio::test]
    async fn suggest_unknown_prefix_returns_empty() {
        let engine = MockEngine::default();

        let suggestions = engine.suggest(query("zzz", 8)).await.unwrap();

        assert!(suggestions.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn suggest_with_latency_completes_after_latency_elapses() {
        // Arrange
        let latency = Duration::from_millis(100);
        let engine = MockEngine::with_latency(latency);
        let started = tokio::time::Instant::now();

        // Act
        let suggestions = engine.suggest(query("rust", 8)).await.unwrap();

        // Assert
        assert_eq!(suggestions.len(), 2);
        assert!(started.elapsed() >= latency);
    }
}
