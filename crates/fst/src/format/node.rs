//! Node encoding.
//!
//! STUB (docs/adr/0006): naive fixed-width layout so the builder can emit real
//! bytes. The compact encoding (varints, relative addresses, specialized
//! one-transition states) replaces it together with the decoder, backed by benchmarks.

use super::NodeAddr;
use crate::output::Output;

const FINAL_FLAG: u8 = 0b0000_0001;

/// A frozen node as the builder hands it to the writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NodeRecord<O> {
    pub(crate) is_final: bool,
    /// Output added when a key ends at this node.
    pub(crate) final_output: O,
    /// Sorted by `byte`, at most one per byte value.
    pub(crate) transitions: Vec<TransitionRecord<O>>,
}

/// An edge to an already written node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransitionRecord<O> {
    pub(crate) byte: u8,
    pub(crate) output: O,
    pub(crate) target: NodeAddr,
}

impl<O: Output> NodeRecord<O> {
    /// A non-final node without transitions.
    pub(crate) fn empty() -> Self {
        Self {
            is_final: false,
            final_output: O::ZERO,
            transitions: Vec::new(),
        }
    }
}

/// Appends the encoding of `node` to `out`.
pub(crate) fn encode_node<O: Output>(node: &NodeRecord<O>, out: &mut Vec<u8>) {
    out.push(if node.is_final { FINAL_FLAG } else { 0 });
    out.extend(node.final_output.to_raw().to_le_bytes());
    let count = u16::try_from(node.transitions.len())
        .expect("a node has at most 256 transitions, one per byte value");
    out.extend(count.to_le_bytes());
    node.transitions
        .iter()
        .for_each(|t| encode_transition(t, out));
}

fn encode_transition<O: Output>(transition: &TransitionRecord<O>, out: &mut Vec<u8>) {
    out.push(transition.byte);
    out.extend(transition.output.to_raw().to_le_bytes());
    out.extend(transition.target.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::MinCost;

    #[test]
    fn encode_node_empty_node_writes_flags_output_and_count() {
        // Arrange
        let mut out: Vec<u8> = Vec::new();

        // Act
        encode_node(&NodeRecord::<MinCost>::empty(), &mut out);

        // Assert
        assert_eq!(out.len(), 1 + 8 + 2);
        assert_eq!(out[0], 0);
    }

    #[test]
    fn encode_node_final_node_with_transition_writes_each_transition() {
        // Arrange
        let node = NodeRecord {
            is_final: true,
            final_output: MinCost::from_raw(1),
            transitions: vec![TransitionRecord {
                byte: b'a',
                output: MinCost::from_raw(2),
                target: 11,
            }],
        };
        let mut out: Vec<u8> = Vec::new();

        // Act
        encode_node(&node, &mut out);

        // Assert
        assert_eq!(out[0], FINAL_FLAG);
        assert_eq!(out.len(), 1 + 8 + 2 + (1 + 8 + 8));
        assert_eq!(out[11], b'a');
    }
}
