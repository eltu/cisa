//! Nodes of the most recently inserted key that may still gain transitions.
//!
//! STUB (docs/adr/0006): only the root is kept and keys are not materialized.
//! The real version holds one node per byte of the last key, freezes the
//! deepest ones as soon as a new key diverges, and pushes outputs toward the root.

use crate::{
    format::{NodeAddr, NodeRecord},
    output::Output,
};

/// Stack of unfrozen nodes along the path of the last inserted key.
#[derive(Debug)]
pub(crate) struct UnfinishedNodes<O> {
    root: NodeRecord<O>,
}

impl<O: Output> UnfinishedNodes<O> {
    pub(crate) fn new() -> Self {
        Self {
            root: NodeRecord::empty(),
        }
    }

    /// Appends one node per byte of `suffix` and attaches `output` to the path.
    pub(crate) fn push_suffix(&mut self, suffix: &[u8], output: O) {
        // STUB (docs/adr/0006): nothing is materialized yet.
        let _ = (suffix, output);
    }

    /// Removes the deepest node below `depth`, ready to be frozen.
    pub(crate) fn pop_deeper_than(&mut self, depth: usize) -> Option<NodeRecord<O>> {
        // STUB (docs/adr/0006): no node below the root exists yet.
        let _ = depth;
        None
    }

    /// Points the last transition of the new deepest node at a frozen node.
    pub(crate) fn link_last_transition(&mut self, target: NodeAddr) {
        // STUB (docs/adr/0006): unreachable until pop_deeper_than yields nodes.
        let _ = target;
    }

    /// Hands over the root once every other node is frozen.
    pub(crate) fn into_root(self) -> NodeRecord<O> {
        self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::MinCost;

    #[test]
    fn into_root_after_push_returns_root_node() {
        // Arrange
        let mut nodes: UnfinishedNodes<MinCost> = UnfinishedNodes::new();
        nodes.push_suffix(b"bolo", MinCost::from_weight(1));
        nodes.link_last_transition(0);

        // Act
        let popped = nodes.pop_deeper_than(0);
        let root = nodes.into_root();

        // Assert
        assert_eq!(popped, None);
        assert_eq!(root, NodeRecord::empty());
    }
}
