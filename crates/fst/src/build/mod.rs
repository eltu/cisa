//! Incremental construction from sorted keys (Daciuk et al., 2000) with output
//! pushing (Mihov & Maurel, 2001).
//!
//! Each inserted key freezes the suffix of the previous key that no later key
//! can extend; frozen nodes go through the [`Registry`] (minimization) and are
//! streamed to the sink by the [`NodeWriter`]. Memory stays proportional to the
//! longest key plus the registry, not to the corpus.

mod node_writer;
mod registry;
mod unfinished;

use std::io::Write;

use node_writer::NodeWriter;
use registry::Registry;
use unfinished::UnfinishedNodes;

use crate::{error::BuildError, output::Output};

/// Streams an FST into `W` from keys inserted in strictly ascending byte order.
///
/// # Examples
///
/// ```
/// use fst::{FstBuilder, MinCost};
///
/// let mut builder = FstBuilder::new(Vec::new()).unwrap();
/// builder.insert(b"bolo", MinCost::from_weight(5)).unwrap();
/// builder.insert(b"bolo de fuba", MinCost::from_weight(2)).unwrap();
/// let bytes: Vec<u8> = builder.finish().unwrap();
/// ```
#[derive(Debug)]
pub struct FstBuilder<W: Write, O: Output> {
    writer: NodeWriter<W>,
    unfinished: UnfinishedNodes<O>,
    registry: Registry,
    last_key: Option<Vec<u8>>,
    inserted: u64,
}

impl<W: Write, O: Output> FstBuilder<W, O> {
    /// Writes the header to `sink` and starts an empty FST.
    pub fn new(sink: W) -> Result<Self, BuildError> {
        Ok(Self {
            writer: NodeWriter::new(sink, O::KIND)?,
            unfinished: UnfinishedNodes::new(),
            registry: Registry,
            last_key: None,
            inserted: 0,
        })
    }

    /// Adds `key` with `output`; `key` must be greater than every key inserted before.
    pub fn insert(&mut self, key: &[u8], output: O) -> Result<(), BuildError> {
        let shared: usize = self.shared_prefix_with_last(key)?;
        self.freeze_deeper_than(shared)?;
        self.unfinished.push_suffix(&key[shared..], output);
        self.remember_last(key);
        Ok(())
    }

    /// Freezes the remaining nodes, writes the footer and returns the sink.
    pub fn finish(mut self) -> Result<W, BuildError> {
        self.freeze_deeper_than(0)?;
        let root = self.unfinished.into_root();
        let root_addr = self.registry.find_or_insert(&root, &mut self.writer)?;
        Ok(self.writer.finish(root_addr)?)
    }

    fn shared_prefix_with_last(&self, key: &[u8]) -> Result<usize, BuildError> {
        let Some(previous) = &self.last_key else {
            return Ok(0);
        };
        if key <= previous.as_slice() {
            return Err(BuildError::KeyOutOfOrder {
                position: self.inserted,
                previous_len: previous.len(),
                key_len: key.len(),
            });
        }
        Ok(common_prefix_len(previous, key))
    }

    fn freeze_deeper_than(&mut self, depth: usize) -> Result<(), BuildError> {
        while let Some(node) = self.unfinished.pop_deeper_than(depth) {
            let addr = self.registry.find_or_insert(&node, &mut self.writer)?;
            self.unfinished.link_last_transition(addr);
        }
        Ok(())
    }

    fn remember_last(&mut self, key: &[u8]) {
        // Reuses the buffer: one allocation for the whole build instead of one per key.
        let last: &mut Vec<u8> = self.last_key.get_or_insert_with(Vec::new);
        last.clear();
        last.extend_from_slice(key);
        self.inserted += 1;
    }
}

fn common_prefix_len(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::{format::decode_envelope, output::MinCost};

    fn builder() -> FstBuilder<Vec<u8>, MinCost> {
        FstBuilder::new(Vec::new()).unwrap()
    }

    #[rstest]
    #[case(b"", b"", 0)]
    #[case(b"bolo", b"bola", 3)]
    #[case(b"bolo", b"bolo de fuba", 4)]
    #[case(b"abc", b"xyz", 0)]
    fn common_prefix_len_pairs_return_shared_byte_count(
        #[case] a: &[u8],
        #[case] b: &[u8],
        #[case] expected: usize,
    ) {
        assert_eq!(common_prefix_len(a, b), expected);
    }

    #[test]
    fn insert_ascending_keys_succeeds() {
        // Arrange
        let mut builder = builder();

        // Act
        let first = builder.insert(b"", MinCost::from_weight(1));
        let second = builder.insert(b"bolo", MinCost::from_weight(2));

        // Assert
        assert!(first.is_ok());
        assert!(second.is_ok());
    }

    #[rstest]
    #[case(b"bolo")]
    #[case(b"abc")]
    fn insert_key_not_greater_than_previous_returns_key_out_of_order(#[case] key: &[u8]) {
        // Arrange
        let mut builder = builder();
        builder.insert(b"bolo", MinCost::from_weight(1)).unwrap();

        // Act
        let result = builder.insert(key, MinCost::from_weight(1));

        // Assert
        assert!(matches!(
            result,
            Err(BuildError::KeyOutOfOrder {
                position: 1,
                previous_len: 4,
                ..
            })
        ));
    }

    #[test]
    fn finish_without_keys_writes_readable_envelope() {
        // Act
        let bytes: Vec<u8> = builder().finish().unwrap();

        // Assert
        assert!(decode_envelope(&bytes, MinCost::KIND).is_ok());
    }

    #[test]
    fn new_failing_sink_returns_io_error() {
        // Arrange
        let sink: &mut [u8] = &mut [];

        // Act
        let result = FstBuilder::<_, MinCost>::new(sink);

        // Assert
        assert!(matches!(result, Err(BuildError::Io(_))));
    }
}
