//! Offline pipeline: corpus -> normalize -> encode key -> sort -> dedup -> FST bytes.

use fst::{FstBuilder, MinCost};

use super::{FstEngineError, key_codec::encode_key};
use crate::autocomplete::normalize::normalize_query;

/// One query of the corpus with its popularity weight (higher is better).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    pub text: String,
    pub weight: u64,
}

/// Builds the index bytes served by [`super::FstEngine`].
///
/// # Examples
///
/// ```
/// use cisa::autocomplete::{IndexEntry, build_index};
///
/// let entries = [IndexEntry { text: "Previsão do tempo".into(), weight: 42 }];
/// let bytes: Vec<u8> = build_index(&entries).unwrap();
/// ```
pub fn build_index(entries: &[IndexEntry]) -> Result<Vec<u8>, FstEngineError> {
    let mut builder = FstBuilder::new(Vec::new())?;
    for (key, cost) in sorted_unique_keys(entries) {
        builder.insert(&key, cost)?;
    }
    Ok(builder.finish()?)
}

/// Keys in the strictly ascending order the builder requires, keeping the best
/// cost of duplicates. Surfaces that differ only in case stay separate keys;
/// collapsing them is a ranking decision for the engine iteration.
fn sorted_unique_keys(entries: &[IndexEntry]) -> Vec<(Vec<u8>, MinCost)> {
    let mut keyed: Vec<(Vec<u8>, MinCost)> = entries
        .iter()
        .map(|entry| (index_key(&entry.text), MinCost::from_weight(entry.weight)))
        .collect();
    // Same key sorts by ascending cost, so dedup keeps the most popular entry.
    keyed.sort_unstable();
    keyed.dedup_by(|later, kept| later.0 == kept.0);
    keyed
}

fn index_key(text: &str) -> Vec<u8> {
    encode_key(&normalize_query(text), text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(text: &str, weight: u64) -> IndexEntry {
        IndexEntry {
            text: text.to_owned(),
            weight,
        }
    }

    #[test]
    fn sorted_unique_keys_unsorted_entries_returns_ascending_keys() {
        // Arrange
        let entries = [entry("rust lang", 1), entry("bolo", 1), entry("rust", 1)];

        // Act
        let keys: Vec<Vec<u8>> = sorted_unique_keys(&entries)
            .into_iter()
            .map(|(k, _)| k)
            .collect();

        // Assert
        assert_eq!(
            keys,
            [b"bolo".to_vec(), b"rust".to_vec(), b"rust lang".to_vec()]
        );
    }

    #[test]
    fn sorted_unique_keys_duplicate_text_keeps_highest_weight() {
        // Arrange
        let entries = [entry("rust", 3), entry("rust", 9), entry("rust", 5)];

        // Act
        let keyed = sorted_unique_keys(&entries);

        // Assert
        assert_eq!(keyed, [(b"rust".to_vec(), MinCost::from_weight(9))]);
    }

    #[test]
    fn build_index_entries_returns_loadable_bytes() {
        // Act
        let bytes = build_index(&[entry("Rust", 1), entry("rust", 2)]).unwrap();

        // Assert
        assert!(crate::autocomplete::FstEngine::from_bytes(bytes).is_ok());
    }
}
