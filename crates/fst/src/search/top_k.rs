//! Top-k completions by output.
//!
//! STUB (docs/adr/0006): sorts the candidates a full search returned. The real
//! version is a best-first search over pushed `MinCost` outputs with a bounded
//! heap: the first `k` final states popped are the exact top-k, in
//! O(k · key length · log k) regardless of how many keys share the prefix.

use super::Completion;
use crate::output::Output;

/// Bounds of a top-k search; both cap work, so latency stays predictable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopKParams {
    /// Maximum completions returned.
    pub k: usize,
    /// Maximum states the search may visit before returning what it has.
    pub max_expansions: usize,
}

/// Keeps the `params.k` lowest outputs, ties broken by key.
pub(crate) fn select_top_k<O: Output>(
    mut candidates: Vec<Completion<O>>,
    params: TopKParams,
) -> Vec<Completion<O>> {
    candidates.truncate(params.max_expansions);
    candidates.sort_unstable_by(|a, b| a.output.cmp(&b.output).then_with(|| a.key.cmp(&b.key)));
    candidates.truncate(params.k);
    candidates
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::output::MinCost;

    fn candidates() -> Vec<Completion<MinCost>> {
        [(b"a", 3), (b"b", 1), (b"c", 2)]
            .into_iter()
            .map(|(key, raw)| Completion {
                key: key.to_vec(),
                output: MinCost::from_raw(raw),
            })
            .collect()
    }

    #[rstest]
    #[case(TopKParams { k: 2, max_expansions: 10 }, vec![b"b", b"c"])]
    #[case(TopKParams { k: 0, max_expansions: 10 }, vec![])]
    #[case(TopKParams { k: 10, max_expansions: 1 }, vec![b"a"])]
    fn select_top_k_params_bound_the_result(
        #[case] params: TopKParams,
        #[case] expected: Vec<&[u8; 1]>,
    ) {
        // Act
        let best = select_top_k(candidates(), params);

        // Assert
        let keys: Vec<&[u8]> = best.iter().map(|c| c.key.as_slice()).collect();
        let expected: Vec<&[u8]> = expected.into_iter().map(|k| k.as_slice()).collect();
        assert_eq!(keys, expected);
    }
}
