//! Output algebra: how values attached to transitions combine along a path.

/// Tag stored in the file header so a reader never decodes outputs with the wrong algebra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OutputKind {
    MinCost = 1,
}

impl OutputKind {
    /// Parses a header tag, or `None` for a tag this version does not know.
    pub fn from_tag(tag: u8) -> Option<Self> {
        (tag == Self::MinCost as u8).then_some(Self::MinCost)
    }

    /// The byte written in the header.
    pub fn tag(self) -> u8 {
        self as u8
    }
}

/// Values carried by transitions, with the operations output pushing needs.
///
/// While building, the shared part of two outputs ([`Output::common`]) moves
/// toward the root; reading a key [`Output::concat`]s the outputs on its path.
/// Ordering matters for search: smaller is better.
///
/// # Examples
///
/// ```
/// use fst::{MinCost, Output};
///
/// let a = MinCost::from_raw(3);
/// let b = MinCost::from_raw(5);
/// let shared = a.common(b);
/// assert_eq!(shared.concat(b.strip(shared)), b);
/// ```
pub trait Output: Copy + Ord + std::fmt::Debug + Send + Sync + 'static {
    /// Header tag for this algebra.
    const KIND: OutputKind;
    /// Identity of [`Output::concat`]: the output of a transition that adds nothing.
    const ZERO: Self;

    /// The part shared by both outputs, pushed toward the root.
    fn common(self, other: Self) -> Self;
    /// Accumulates two consecutive outputs along a path.
    fn concat(self, other: Self) -> Self;
    /// Removes a previously pushed `prefix` from `self`.
    fn strip(self, prefix: Self) -> Self;
    /// Fixed-width representation stored on disk.
    fn to_raw(self) -> u64;
    /// Inverse of [`Output::to_raw`].
    fn from_raw(raw: u64) -> Self;
}

/// Cost in the tropical semiring (min, +): after pushing, the output read up to
/// a node is the cost of the best key below it, which makes top-k exact.
///
/// # Examples
///
/// ```
/// use fst::MinCost;
///
/// let popular = MinCost::from_weight(100);
/// let rare = MinCost::from_weight(1);
/// assert!(popular < rare);
/// assert_eq!(popular.weight(), 100);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MinCost(u64);

impl MinCost {
    /// Converts a popularity weight (higher is better) into a cost (lower is better).
    pub const fn from_weight(weight: u64) -> Self {
        Self(u64::MAX - weight)
    }

    /// Inverse of [`MinCost::from_weight`].
    pub const fn weight(self) -> u64 {
        u64::MAX - self.0
    }
}

impl Output for MinCost {
    const KIND: OutputKind = OutputKind::MinCost;
    const ZERO: Self = Self(0);

    fn common(self, other: Self) -> Self {
        self.min(other)
    }

    fn concat(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    fn strip(self, prefix: Self) -> Self {
        Self(self.0.saturating_sub(prefix.0))
    }

    fn to_raw(self) -> u64 {
        self.0
    }

    fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_kind_from_tag_round_trips_known_tag() {
        assert_eq!(
            OutputKind::from_tag(OutputKind::MinCost.tag()),
            Some(OutputKind::MinCost)
        );
    }

    #[test]
    fn output_kind_from_tag_unknown_tag_returns_none() {
        assert_eq!(OutputKind::from_tag(0), None);
    }

    #[test]
    fn min_cost_from_weight_higher_weight_sorts_first() {
        assert!(MinCost::from_weight(10) < MinCost::from_weight(9));
    }

    #[test]
    fn min_cost_common_returns_smaller_cost() {
        assert_eq!(MinCost(3).common(MinCost(7)), MinCost(3));
    }

    #[test]
    fn min_cost_strip_then_concat_restores_original() {
        // Arrange
        let original = MinCost(7);
        let pushed = MinCost(3);

        // Act
        let restored = pushed.concat(original.strip(pushed));

        // Assert
        assert_eq!(restored, original);
    }

    #[test]
    fn min_cost_concat_overflow_saturates() {
        assert_eq!(MinCost(u64::MAX).concat(MinCost(1)), MinCost(u64::MAX));
    }

    #[test]
    fn min_cost_zero_is_concat_identity() {
        assert_eq!(MinCost(5).concat(MinCost::ZERO), MinCost(5));
    }
}
