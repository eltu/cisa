use crate::{format::MAGIC, output::OutputKind};

/// Why an FST could not be built.
///
/// Key contents are never included: keys may hold personal data (search
/// queries), so errors carry positions and lengths instead.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// Keys must arrive in strictly ascending byte order (no duplicates).
    #[error(
        "key #{position} ({key_len} bytes) is not greater than the previous key \
         ({previous_len} bytes); keys must be inserted in strictly ascending byte order"
    )]
    KeyOutOfOrder {
        position: u64,
        previous_len: usize,
        key_len: usize,
    },

    /// The sink rejected a write.
    #[error("writing the FST failed: {0}")]
    Io(#[from] std::io::Error),
}

/// Why a byte slice is not a readable FST.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FormatError {
    /// Shorter than the fixed header plus footer.
    #[error("FST is {len} bytes; expected at least {min} bytes (header + footer)")]
    TooShort { len: usize, min: usize },

    /// The file does not start with the cisa FST magic.
    #[error("FST starts with {found:?}; expected the magic {MAGIC:?}")]
    BadMagic { found: Vec<u8> },

    /// Written by an incompatible version of this crate.
    #[error("FST format version {found} is unsupported; expected version {expected}")]
    UnsupportedVersion { found: u16, expected: u16 },

    /// The file stores a different output algebra than the one requested.
    #[error("FST output kind tag {found} does not match the requested {expected:?}")]
    OutputKindMismatch { found: u8, expected: OutputKind },

    /// The footer points outside the node area.
    #[error("FST root address {root} is out of bounds; expected an offset below {limit}")]
    RootOutOfBounds { root: u64, limit: u64 },
}
