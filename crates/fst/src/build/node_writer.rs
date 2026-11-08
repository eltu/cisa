//! Streams encoded nodes to the sink and tracks their addresses.

use std::io::{self, Write};

use crate::{
    format::{self, NodeAddr, NodeRecord},
    output::{Output, OutputKind},
};

/// Owns the sink: header first, then nodes in post-order, then the footer.
#[derive(Debug)]
pub(crate) struct NodeWriter<W> {
    sink: W,
    position: u64,
    // Reused for every node, so encoding does not allocate per node.
    buffer: Vec<u8>,
}

impl<W: Write> NodeWriter<W> {
    pub(crate) fn new(mut sink: W, kind: OutputKind) -> io::Result<Self> {
        let header = format::encode_header(kind);
        sink.write_all(&header)?;
        Ok(Self {
            sink,
            position: header.len() as u64,
            buffer: Vec::new(),
        })
    }

    /// Writes `node` and returns the address it was written at.
    pub(crate) fn write_node<O: Output>(&mut self, node: &NodeRecord<O>) -> io::Result<NodeAddr> {
        self.buffer.clear();
        format::encode_node(node, &mut self.buffer);
        self.sink.write_all(&self.buffer)?;
        let addr: NodeAddr = self.position;
        self.position += self.buffer.len() as u64;
        Ok(addr)
    }

    /// Writes the footer pointing at `root` and returns the flushed sink.
    pub(crate) fn finish(mut self, root: NodeAddr) -> io::Result<W> {
        self.sink.write_all(&format::encode_footer(root))?;
        self.sink.flush()?;
        Ok(self.sink)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        format::{FOOTER_LEN, HEADER_LEN, decode_envelope},
        output::MinCost,
    };

    #[test]
    fn write_node_consecutive_nodes_return_increasing_addresses() {
        // Arrange
        let mut writer = NodeWriter::new(Vec::new(), MinCost::KIND).unwrap();
        let node = NodeRecord::<MinCost>::empty();

        // Act
        let first = writer.write_node(&node).unwrap();
        let second = writer.write_node(&node).unwrap();

        // Assert
        assert_eq!(first, HEADER_LEN as u64);
        assert!(second > first);
    }

    #[test]
    fn finish_after_root_writes_footer_pointing_at_root() {
        // Arrange
        let mut writer = NodeWriter::new(Vec::new(), MinCost::KIND).unwrap();
        let root = writer.write_node(&NodeRecord::<MinCost>::empty()).unwrap();

        // Act
        let bytes: Vec<u8> = writer.finish(root).unwrap();

        // Assert
        assert_eq!(decode_envelope(&bytes, MinCost::KIND), Ok(root));
        assert!(bytes.len() > root as usize + FOOTER_LEN);
    }
}
