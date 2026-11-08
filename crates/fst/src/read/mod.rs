//! Read-only, zero-copy access to an FST stored in any byte container.

mod fixture;

use std::{fmt, marker::PhantomData};

use crate::{
    error::FormatError,
    format::{self, NodeAddr},
    output::Output,
    search::{self, Automaton, Completion, Prefix, TopKParams},
};

/// An FST over bytes held by `D` (`Vec<u8>` today, a memory map later).
///
/// # Examples
///
/// ```
/// use fst::{Fst, FstBuilder, MinCost};
///
/// let bytes: Vec<u8> = FstBuilder::<_, MinCost>::new(Vec::new()).unwrap().finish().unwrap();
/// let fst: Fst<Vec<u8>, MinCost> = Fst::open(bytes).unwrap();
/// let _ = fst.get(b"rust");
/// ```
pub struct Fst<D, O> {
    data: D,
    root: NodeAddr,
    output: PhantomData<O>,
}

impl<D: AsRef<[u8]>, O: Output> Fst<D, O> {
    /// Validates the header and footer of `data`; nodes are decoded lazily.
    pub fn open(data: D) -> Result<Self, FormatError> {
        let root = format::decode_envelope(data.as_ref(), O::KIND)?;
        Ok(Self {
            data,
            root,
            output: PhantomData,
        })
    }

    /// The output of `key`, or `None` when `key` is not in the FST.
    pub fn get(&self, key: &[u8]) -> Option<O> {
        // STUB (docs/adr/0006): looks up the fixture instead of walking nodes from the root.
        fixture::output_of(key)
    }

    /// Every key accepted by `automaton`, in ascending byte order.
    pub fn search<A: Automaton>(&self, automaton: &A) -> Vec<Completion<O>> {
        // STUB (docs/adr/0006): scans the fixture instead of intersecting the automaton with the nodes.
        fixture::completions()
            .filter(|completion| search::accepts(automaton, &completion.key))
            .collect()
    }

    /// The `params.k` keys starting with `prefix` that have the lowest output.
    pub fn top_k(&self, prefix: &[u8], params: TopKParams) -> Vec<Completion<O>> {
        search::select_top_k(self.search(&Prefix::new(prefix)), params)
    }
}

// Manual impl: the derived one would dump every byte of the FST into logs.
impl<D: AsRef<[u8]>, O> fmt::Debug for Fst<D, O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fst")
            .field("len", &self.data.as_ref().len())
            .field("root", &self.root)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build::FstBuilder, output::MinCost};

    fn fst() -> Fst<Vec<u8>, MinCost> {
        let bytes: Vec<u8> = FstBuilder::<_, MinCost>::new(Vec::new())
            .unwrap()
            .finish()
            .unwrap();
        Fst::open(bytes).unwrap()
    }

    fn keys(completions: &[Completion<MinCost>]) -> Vec<&[u8]> {
        completions.iter().map(|c| c.key.as_slice()).collect()
    }

    #[test]
    fn open_garbage_bytes_returns_format_error() {
        let result = Fst::<_, MinCost>::open(b"definitely not a cisa fst file".to_vec());

        assert!(matches!(result, Err(FormatError::BadMagic { .. })));
    }

    #[test]
    fn get_fixture_key_returns_its_output() {
        assert!(fst().get(b"rust").is_some());
    }

    #[test]
    fn get_missing_key_returns_none() {
        assert_eq!(fst().get(b"zzz"), None);
    }

    #[test]
    fn search_prefix_returns_keys_in_ascending_order() {
        // Act
        let found = fst().search(&Prefix::new(b"rust "));

        // Assert
        assert_eq!(keys(&found), [b"rust async".as_slice(), b"rust lang"]);
    }

    #[test]
    fn top_k_prefix_returns_lowest_outputs_first() {
        // Act
        let best = fst().top_k(
            b"rust",
            TopKParams {
                k: 2,
                max_expansions: 100,
            },
        );

        // Assert
        assert_eq!(keys(&best), [b"rust async".as_slice(), b"rust lang"]);
    }

    #[test]
    fn debug_omits_raw_bytes() {
        let rendered = format!("{:?}", fst());

        assert!(rendered.starts_with("Fst { len: "));
    }
}
