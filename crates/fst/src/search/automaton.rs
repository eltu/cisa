//! Deterministic automata a search runs in lockstep with the FST.
//!
//! New query kinds (e.g. a Levenshtein automaton for typo tolerance) are new
//! [`Automaton`] implementations; builder, format and reader stay untouched.

/// A deterministic automaton over bytes.
///
/// # Examples
///
/// ```
/// use fst::{Automaton, Prefix};
///
/// let prefix = Prefix::new(b"ru");
/// let state = prefix.accept(&prefix.start(), b'r');
/// assert!(prefix.can_match(&state));
/// assert!(!prefix.is_match(&state));
/// ```
pub trait Automaton {
    /// Where the automaton is after reading some bytes.
    type State: Clone;

    /// State before reading any byte.
    fn start(&self) -> Self::State;
    /// State after reading `byte` in `state`.
    fn accept(&self, state: &Self::State, byte: u8) -> Self::State;
    /// Whether the bytes read so far form an accepted key.
    fn is_match(&self, state: &Self::State) -> bool;
    /// Whether some continuation can still be accepted; lets the search prune.
    fn can_match(&self, state: &Self::State) -> bool;
}

/// Accepts every key starting with a byte prefix.
#[derive(Debug, Clone, Copy)]
pub struct Prefix<'a> {
    prefix: &'a [u8],
}

impl<'a> Prefix<'a> {
    /// An automaton accepting keys that start with `prefix`.
    pub fn new(prefix: &'a [u8]) -> Self {
        Self { prefix }
    }
}

impl Automaton for Prefix<'_> {
    /// Bytes of the prefix matched so far, `None` once the key diverged.
    type State = Option<usize>;

    fn start(&self) -> Self::State {
        Some(0)
    }

    fn accept(&self, state: &Self::State, byte: u8) -> Self::State {
        let matched: usize = (*state)?;
        let Some(&expected) = self.prefix.get(matched) else {
            return Some(matched);
        };
        (byte == expected).then_some(matched + 1)
    }

    fn is_match(&self, state: &Self::State) -> bool {
        *state == Some(self.prefix.len())
    }

    fn can_match(&self, state: &Self::State) -> bool {
        state.is_some()
    }
}

/// Runs `automaton` over `key`, stopping as soon as no match is possible.
pub(crate) fn accepts<A: Automaton>(automaton: &A, key: &[u8]) -> bool {
    let mut state: A::State = automaton.start();
    for &byte in key {
        if !automaton.can_match(&state) {
            return false;
        }
        state = automaton.accept(&state, byte);
    }
    automaton.is_match(&state)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(b"", b"", true)]
    #[case(b"", b"rust", true)]
    #[case(b"ru", b"rust", true)]
    #[case(b"rust", b"rust", true)]
    #[case(b"rust", b"ru", false)]
    #[case(b"rx", b"rust", false)]
    fn accepts_prefix_against_key_returns_whether_key_starts_with_prefix(
        #[case] prefix: &[u8],
        #[case] key: &[u8],
        #[case] expected: bool,
    ) {
        assert_eq!(accepts(&Prefix::new(prefix), key), expected);
    }

    #[test]
    fn accept_after_divergence_stays_dead() {
        // Arrange
        let prefix = Prefix::new(b"ab");
        let dead = prefix.accept(&prefix.start(), b'x');

        // Act
        let still_dead = prefix.accept(&dead, b'b');

        // Assert
        assert!(!prefix.can_match(&still_dead));
    }
}
