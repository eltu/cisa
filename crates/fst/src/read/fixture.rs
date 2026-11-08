//! STUB (docs/adr/0006): fixed entries every [`super::Fst`] answers from until
//! node traversal lands. Delete this file when `get`/`search` walk real nodes.

use crate::{output::Output, search::Completion};

/// Sorted keys with raw outputs (lower is better).
const STUB_ENTRIES: &[(&[u8], u64)] = &[
    (b"rust", 30),
    (b"rust async", 10),
    (b"rust lang", 20),
    (b"rustup", 40),
];

pub(super) fn output_of<O: Output>(key: &[u8]) -> Option<O> {
    let index: usize = STUB_ENTRIES.binary_search_by(|(k, _)| (*k).cmp(key)).ok()?;
    Some(O::from_raw(STUB_ENTRIES[index].1))
}

pub(super) fn completions<O: Output>() -> impl Iterator<Item = Completion<O>> {
    STUB_ENTRIES.iter().map(|(key, raw)| Completion {
        key: key.to_vec(),
        output: O::from_raw(*raw),
    })
}
