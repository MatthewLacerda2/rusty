//! src/procgen/recipe/arity.rs — how many inputs an op accepts (#1485).
//!
//! Each [`super::OPS`] row carries one; [`super::Node::check_arity`] holds a node's
//! `inputs` to it, so a dangling or surplus input is an error, never a black map.

/// How many inputs an op accepts: `min..=max`. Optional inputs are the trailing
/// ones (`mix`'s mask, `combine_rgb`'s G and B channels).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arity {
    pub min: u8,
    pub max: u8,
}

impl Arity {
    /// Whether a node wired with `given` inputs is accepted.
    pub fn accepts(self, given: usize) -> bool {
        (usize::from(self.min)..=usize::from(self.max)).contains(&given)
    }
}

impl std::fmt::Display for Arity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.min, self.max) {
            (min, max) if min == max => write!(f, "{min}"),
            (min, max) => write!(f, "{min} to {max}"),
        }
    }
}

pub(super) const fn arity(min: u8, max: u8) -> Arity {
    Arity { min, max }
}

/// A generator: no inputs.
pub(super) const SOURCE: Arity = arity(0, 0);
/// A single-input op.
pub(super) const ONE: Arity = arity(1, 1);
