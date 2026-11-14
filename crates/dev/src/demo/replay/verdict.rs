//! What a replay found (#1581): how far it got, and the first digest that differed.

/// The first digest that differs from the recording: the state went astray after
/// `matched` (the last digest that agreed, if any) and by `tick`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divergence {
    pub tick: u64,
    pub matched: Option<u64>,
    pub expected: String,
    pub actual: String,
}

impl std::fmt::Display for Divergence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "diverged by tick {}", self.tick)?;
        match self.matched {
            Some(t) => write!(f, " (the state still matched at tick {t})"),
            None => write!(f, " (no earlier digest matched)"),
        }
    }
}

/// How a replay went, so far.
#[derive(Clone, Debug, Default)]
pub struct Verdict {
    /// Ticks replayed.
    pub ticks: u64,
    /// Ticks the demo holds.
    pub of: u64,
    /// Digests compared.
    pub checked: u64,
    /// The last tick whose digest agreed.
    pub matched: Option<u64>,
    pub diverged: Option<Divergence>,
    /// `(tick, what)` for every act the demo could not carry.
    pub taints: Vec<(u64, String)>,
    /// Whether a scenario on top steps the replay: it may end it before the
    /// demo's end on purpose.
    pub scenario: bool,
}
