//! Minimal acyclic finite state transducer (FST) mapping byte keys to outputs.
//!
//! The crate knows nothing about autocomplete, text or async runtimes: keys are
//! bytes, outputs follow the [`Output`] algebra. Writing ([`FstBuilder`]) and
//! reading ([`Fst`]) only share the binary contract in `format`.
//!
//! STUB (docs/adr/0006): the construction and traversal algorithms are not
//! implemented yet. The builder writes a valid file but no shared structure,
//! and every [`Fst`] answers from a fixed table of entries.
//!
//! # Examples
//!
//! ```
//! use fst::{Fst, FstBuilder, MinCost, TopKParams};
//!
//! let mut builder = FstBuilder::new(Vec::new()).unwrap();
//! builder.insert(b"rust", MinCost::from_weight(10)).unwrap();
//! let bytes: Vec<u8> = builder.finish().unwrap();
//!
//! let fst: Fst<Vec<u8>, MinCost> = Fst::open(bytes).unwrap();
//! let best = fst.top_k(b"ru", TopKParams { k: 3, max_expansions: 1_000 });
//! assert!(best.len() <= 3);
//! ```

#![forbid(unsafe_code)]

mod build;
mod error;
mod format;
mod output;
mod read;
mod search;

pub use build::FstBuilder;
pub use error::{BuildError, FormatError};
pub use output::{MinCost, Output, OutputKind};
pub use read::Fst;
pub use search::{Automaton, Completion, Prefix, TopKParams};
