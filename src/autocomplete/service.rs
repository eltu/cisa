use std::sync::Arc;

use super::{SuggestError, SuggestParams, SuggestQuery, Suggestion, SuggestionEngine};

const REGION_CODE_LEN: usize = 2;
// Longest BCP 47 tags in practical use stay well below this.
const MAX_LOCALE_LEN: usize = 35;

/// Bounds and defaults applied to every suggest request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestLimits {
    pub max_query_bytes: usize,
    pub default_limit: usize,
    pub max_limit: usize,
    pub default_locale: String,
}

impl Default for SuggestLimits {
    fn default() -> Self {
        Self {
            max_query_bytes: 512,
            default_limit: 8,
            max_limit: 10,
            default_locale: "pt-BR".to_owned(),
        }
    }
}

/// Validates suggest requests and delegates them to the configured engine.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use cisa::autocomplete::{MockEngine, SuggestLimits, SuggestParams, SuggestService};
///
/// # tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
/// let service = SuggestService::new(Arc::new(MockEngine::default()), SuggestLimits::default());
/// let params = SuggestParams { query: "como fazer bo".into(), ..Default::default() };
/// let suggestions = service.suggest(params).await.unwrap();
/// assert!(!suggestions.is_empty());
/// # });
/// ```
#[derive(Debug)]
pub struct SuggestService {
    engine: Arc<dyn SuggestionEngine>,
    limits: SuggestLimits,
}

impl SuggestService {
    /// Creates a service backed by `engine` and bounded by `limits`.
    pub fn new(engine: Arc<dyn SuggestionEngine>, limits: SuggestLimits) -> Self {
        Self { engine, limits }
    }

    /// Validates `params` and returns suggestions. A blank query yields no suggestions.
    pub async fn suggest(&self, params: SuggestParams) -> Result<Vec<Suggestion>, SuggestError> {
        let query: SuggestQuery = self.validate(params)?;
        if query.text.trim().is_empty() {
            return Ok(Vec::new());
        }
        self.engine.suggest(query).await
    }

    fn validate(&self, params: SuggestParams) -> Result<SuggestQuery, SuggestError> {
        Ok(SuggestQuery {
            text: validate_query(&params.query, self.limits.max_query_bytes)?,
            locale: self.resolve_locale(&params.locale)?,
            region: validate_region(&params.region)?,
            limit: self.resolve_limit(params.limit),
        })
    }

    fn resolve_locale(&self, raw: &str) -> Result<String, SuggestError> {
        if raw.is_empty() {
            return Ok(self.limits.default_locale.clone());
        }
        let well_formed: bool = raw.len() <= MAX_LOCALE_LEN
            && raw.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        if well_formed {
            return Ok(raw.to_owned());
        }
        Err(SuggestError::InvalidArgument(format!(
            "locale {raw:?} is not a BCP 47 tag like \"pt-BR\""
        )))
    }

    fn resolve_limit(&self, requested: u32) -> usize {
        if requested == 0 {
            return self.limits.default_limit;
        }
        usize::try_from(requested).map_or(self.limits.max_limit, |n| n.min(self.limits.max_limit))
    }
}

// Trailing whitespace is kept on purpose: "bolo " and "bolo" ask for different completions.
fn validate_query(raw: &str, max_bytes: usize) -> Result<String, SuggestError> {
    if raw.len() > max_bytes {
        return Err(SuggestError::InvalidArgument(format!(
            "query is {} bytes; max is {max_bytes}",
            raw.len()
        )));
    }
    Ok(raw.trim_start().to_owned())
}

fn validate_region(raw: &str) -> Result<Option<String>, SuggestError> {
    if raw.is_empty() {
        return Ok(None);
    }
    if raw.len() == REGION_CODE_LEN && raw.chars().all(|c| c.is_ascii_alphabetic()) {
        return Ok(Some(raw.to_ascii_uppercase()));
    }
    Err(SuggestError::InvalidArgument(format!(
        "region {raw:?} is not an ISO 3166-1 alpha-2 code like \"BR\""
    )))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::autocomplete::{EngineFuture, MockEngine};

    #[derive(Debug)]
    struct FailingEngine;

    impl SuggestionEngine for FailingEngine {
        fn suggest(&self, _query: SuggestQuery) -> EngineFuture<'_> {
            Box::pin(async { Err(SuggestError::EngineUnavailable("model offline".into())) })
        }
    }

    fn service_with(engine: impl SuggestionEngine) -> SuggestService {
        SuggestService::new(Arc::new(engine), SuggestLimits::default())
    }

    fn params(query: &str) -> SuggestParams {
        SuggestParams {
            query: query.to_owned(),
            ..SuggestParams::default()
        }
    }

    #[tokio::test]
    async fn suggest_matching_query_returns_engine_suggestions() {
        // Arrange
        let service = service_with(MockEngine::default());

        // Act
        let suggestions = service
            .suggest(params("como fazer bolo de c"))
            .await
            .unwrap();

        // Assert
        let texts: Vec<&str> = suggestions.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(
            texts,
            ["como fazer bolo de cenoura", "como fazer bolo de chocolate"]
        );
    }

    #[rstest]
    #[case("")]
    #[case("   ")]
    #[tokio::test]
    async fn suggest_blank_query_returns_empty_without_calling_engine(#[case] query: &str) {
        let service = service_with(FailingEngine);

        let suggestions = service.suggest(params(query)).await.unwrap();

        assert!(suggestions.is_empty());
    }

    #[tokio::test]
    async fn suggest_engine_failure_propagates_error() {
        let service = service_with(FailingEngine);

        let result = service.suggest(params("bolo")).await;

        assert!(matches!(result, Err(SuggestError::EngineUnavailable(_))));
    }

    #[test]
    fn validate_query_at_byte_limit_is_accepted() {
        let max_bytes = SuggestLimits::default().max_query_bytes;

        let result = validate_query(&"a".repeat(max_bytes), max_bytes);

        assert!(result.is_ok());
    }

    #[test]
    fn validate_query_over_byte_limit_returns_invalid_argument_with_size() {
        let max_bytes = SuggestLimits::default().max_query_bytes;

        let result = validate_query(&"a".repeat(max_bytes + 1), max_bytes);

        assert_eq!(
            result,
            Err(SuggestError::InvalidArgument(
                "query is 513 bytes; max is 512".into()
            ))
        );
    }

    #[test]
    fn validate_query_leading_whitespace_is_trimmed_and_trailing_kept() {
        let text = validate_query("  bolo ", 512).unwrap();

        assert_eq!(text, "bolo ");
    }

    #[rstest]
    #[case("", None)]
    #[case("br", Some("BR"))]
    #[case("US", Some("US"))]
    fn validate_region_valid_input_returns_normalized_code(
        #[case] raw: &str,
        #[case] expected: Option<&str>,
    ) {
        let region = validate_region(raw).unwrap();

        assert_eq!(region.as_deref(), expected);
    }

    #[rstest]
    #[case("B")]
    #[case("BRA")]
    #[case("1R")]
    fn validate_region_malformed_code_returns_invalid_argument(#[case] raw: &str) {
        let result = validate_region(raw);

        assert!(matches!(result, Err(SuggestError::InvalidArgument(_))));
    }

    #[rstest]
    #[case("", "pt-BR")]
    #[case("en-US", "en-US")]
    fn resolve_locale_valid_input_returns_locale(#[case] raw: &str, #[case] expected: &str) {
        let service = service_with(MockEngine::default());

        let locale = service.resolve_locale(raw).unwrap();

        assert_eq!(locale, expected);
    }

    #[rstest]
    #[case("pt_BR")]
    #[case("pt-BR; drop")]
    #[case(&"a".repeat(MAX_LOCALE_LEN + 1))]
    fn resolve_locale_malformed_tag_returns_invalid_argument(#[case] raw: &str) {
        let service = service_with(MockEngine::default());

        let result = service.resolve_locale(raw);

        assert!(matches!(result, Err(SuggestError::InvalidArgument(_))));
    }

    #[rstest]
    #[case(0, 8)]
    #[case(1, 1)]
    #[case(10, 10)]
    #[case(u32::MAX, 10)]
    fn resolve_limit_requested_value_is_defaulted_and_capped(
        #[case] requested: u32,
        #[case] expected: usize,
    ) {
        let service = service_with(MockEngine::default());

        let limit = service.resolve_limit(requested);

        assert_eq!(limit, expected);
    }
}
