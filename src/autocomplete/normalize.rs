//! How a query is folded before it meets the index.
//!
//! The indexer and the engine must fold text identically, otherwise queries
//! silently stop matching; this function is the single source of that rule.

/// Folds `text` into the form used as index key.
///
/// NUL is removed because it separates the normalized form from the surface
/// form inside an index key (see `fst_engine::key_codec`).
pub(crate) fn normalize_query(text: &str) -> String {
    // STUB (docs/adr/0006): lowercase only; accent folding ("previsão" -> "previsao")
    // arrives with the engine iteration.
    text.to_lowercase().replace('\0', "")
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("Previsão", "previsão")]
    #[case("RUST", "rust")]
    #[case("ru\0st", "rust")]
    #[case("", "")]
    fn normalize_query_input_returns_folded_text(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(normalize_query(text), expected);
    }
}
