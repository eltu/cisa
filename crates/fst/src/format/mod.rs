//! Binary contract shared by the builder and the reader.
//!
//! Layout: `header | nodes... | footer`. Nodes are written in post-order
//! (children before parents), so the root is last and its address lives in the
//! footer. Addresses are absolute byte offsets.

mod node;

pub(crate) use node::{NodeRecord, encode_node};

use crate::{error::FormatError, output::OutputKind};

/// First bytes of every cisa FST file.
pub(crate) const MAGIC: &[u8; 8] = b"CISAFST\0";
/// Bumped on any incompatible layout change.
pub(crate) const FORMAT_VERSION: u16 = 1;
/// Magic + version + output kind tag.
pub(crate) const HEADER_LEN: usize = MAGIC.len() + 2 + 1;
/// Root address as little-endian u64.
pub(crate) const FOOTER_LEN: usize = 8;

/// Byte offset of a node inside the file.
pub(crate) type NodeAddr = u64;

/// Serializes the header for an FST holding `kind` outputs.
pub(crate) fn encode_header(kind: OutputKind) -> [u8; HEADER_LEN] {
    let mut header = [0; HEADER_LEN];
    header[..MAGIC.len()].copy_from_slice(MAGIC);
    header[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
    header[HEADER_LEN - 1] = kind.tag();
    header
}

/// Validates header and footer, returning the root address.
pub(crate) fn decode_envelope(bytes: &[u8], expected: OutputKind) -> Result<NodeAddr, FormatError> {
    check_len(bytes)?;
    check_magic(bytes)?;
    check_version(bytes)?;
    check_kind(bytes, expected)?;
    decode_root(bytes)
}

/// Serializes the footer pointing at `root`.
pub(crate) fn encode_footer(root: NodeAddr) -> [u8; FOOTER_LEN] {
    root.to_le_bytes()
}

fn check_len(bytes: &[u8]) -> Result<(), FormatError> {
    let min: usize = HEADER_LEN + FOOTER_LEN;
    if bytes.len() < min {
        return Err(FormatError::TooShort {
            len: bytes.len(),
            min,
        });
    }
    Ok(())
}

fn check_magic(bytes: &[u8]) -> Result<(), FormatError> {
    let found: &[u8] = &bytes[..MAGIC.len()];
    if found != MAGIC {
        return Err(FormatError::BadMagic {
            found: found.to_vec(),
        });
    }
    Ok(())
}

fn check_version(bytes: &[u8]) -> Result<(), FormatError> {
    let found = u16::from_le_bytes([bytes[MAGIC.len()], bytes[MAGIC.len() + 1]]);
    if found != FORMAT_VERSION {
        return Err(FormatError::UnsupportedVersion {
            found,
            expected: FORMAT_VERSION,
        });
    }
    Ok(())
}

fn check_kind(bytes: &[u8], expected: OutputKind) -> Result<(), FormatError> {
    let found: u8 = bytes[HEADER_LEN - 1];
    if OutputKind::from_tag(found) != Some(expected) {
        return Err(FormatError::OutputKindMismatch { found, expected });
    }
    Ok(())
}

fn decode_root(bytes: &[u8]) -> Result<NodeAddr, FormatError> {
    let footer_start: usize = bytes.len() - FOOTER_LEN;
    let footer: [u8; FOOTER_LEN] = bytes[footer_start..]
        .try_into()
        .expect("the slice is FOOTER_LEN long because check_len ran first");
    let root = NodeAddr::from_le_bytes(footer);
    let limit = footer_start as u64;
    if root < HEADER_LEN as u64 || root >= limit {
        return Err(FormatError::RootOutOfBounds { root, limit });
    }
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(header: [u8; HEADER_LEN], node_bytes: usize, root: NodeAddr) -> Vec<u8> {
        let mut bytes: Vec<u8> = header.to_vec();
        bytes.extend(std::iter::repeat_n(0, node_bytes));
        bytes.extend(encode_footer(root));
        bytes
    }

    fn valid() -> Vec<u8> {
        envelope(encode_header(OutputKind::MinCost), 4, HEADER_LEN as u64)
    }

    #[test]
    fn decode_envelope_valid_file_returns_root() {
        assert_eq!(
            decode_envelope(&valid(), OutputKind::MinCost),
            Ok(HEADER_LEN as u64)
        );
    }

    #[test]
    fn decode_envelope_truncated_file_returns_too_short() {
        let result = decode_envelope(&valid()[..5], OutputKind::MinCost);

        assert!(matches!(result, Err(FormatError::TooShort { len: 5, .. })));
    }

    #[test]
    fn decode_envelope_foreign_magic_returns_bad_magic() {
        // Arrange
        let mut bytes = valid();
        bytes[0] = b'X';

        // Act
        let result = decode_envelope(&bytes, OutputKind::MinCost);

        // Assert
        assert!(matches!(result, Err(FormatError::BadMagic { .. })));
    }

    #[test]
    fn decode_envelope_newer_version_returns_unsupported_version() {
        // Arrange
        let mut bytes = valid();
        bytes[MAGIC.len()] = 99;

        // Act
        let result = decode_envelope(&bytes, OutputKind::MinCost);

        // Assert
        assert!(matches!(
            result,
            Err(FormatError::UnsupportedVersion { found: 99, .. })
        ));
    }

    #[test]
    fn decode_envelope_unknown_output_kind_returns_mismatch() {
        // Arrange
        let mut bytes = valid();
        bytes[HEADER_LEN - 1] = 42;

        // Act
        let result = decode_envelope(&bytes, OutputKind::MinCost);

        // Assert
        assert!(matches!(
            result,
            Err(FormatError::OutputKindMismatch { found: 42, .. })
        ));
    }

    #[test]
    fn decode_envelope_root_inside_footer_returns_out_of_bounds() {
        // Arrange
        let bytes = envelope(encode_header(OutputKind::MinCost), 4, 1_000);

        // Act
        let result = decode_envelope(&bytes, OutputKind::MinCost);

        // Assert
        assert!(matches!(
            result,
            Err(FormatError::RootOutOfBounds { root: 1_000, .. })
        ));
    }
}
