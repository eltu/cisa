//! Search algorithms over an FST, expressed as automata intersected with it.

mod automaton;
mod top_k;

pub use automaton::{Automaton, Prefix};
pub use top_k::TopKParams;

pub(crate) use automaton::accepts;
pub(crate) use top_k::select_top_k;

/// A key found by a search, with the output accumulated along its path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion<O> {
    pub key: Vec<u8>,
    pub output: O,
}
