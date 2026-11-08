//! Hash-consing of frozen nodes: equivalent nodes are written once, which is
//! what makes the automaton minimal.
//!
//! STUB (docs/adr/0006): no deduplication yet, every node is written. The real
//! version starts as an exact `HashMap<NodeRecord, NodeAddr>`; a bounded cache
//! (less minimal, less memory) is a later, measured tradeoff.

use std::io::{self, Write};

use super::node_writer::NodeWriter;
use crate::{
    format::{NodeAddr, NodeRecord},
    output::Output,
};

/// Index of frozen nodes already written, keyed by their structure.
#[derive(Debug)]
pub(crate) struct Registry;

impl Registry {
    /// Returns the address of an equivalent written node, writing `node` if none exists.
    pub(crate) fn find_or_insert<W: Write, O: Output>(
        &mut self,
        node: &NodeRecord<O>,
        writer: &mut NodeWriter<W>,
    ) -> io::Result<NodeAddr> {
        writer.write_node(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{format::HEADER_LEN, output::MinCost};

    #[test]
    fn find_or_insert_new_node_returns_written_address() {
        // Arrange
        let mut writer = NodeWriter::new(Vec::new(), MinCost::KIND).unwrap();

        // Act
        let addr = Registry.find_or_insert(&NodeRecord::<MinCost>::empty(), &mut writer);

        // Assert
        assert_eq!(addr.unwrap(), HEADER_LEN as u64);
    }
}
