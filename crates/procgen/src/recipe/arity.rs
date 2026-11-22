/// How many inputs an op accepts: `min..=max`. Optional inputs are the trailing
/// ones (`mix`'s mask, `combine_rgb`'s G and B channels).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arity {
    pub min: u8,
    pub max: u8,
}

impl Arity {
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

pub(crate) const fn arity(min: u8, max: u8) -> Arity {
    Arity { min, max }
}

pub(crate) const SOURCE: Arity = arity(0, 0);
pub(crate) const ONE: Arity = arity(1, 1);
