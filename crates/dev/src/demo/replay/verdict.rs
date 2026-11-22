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

#[derive(Clone, Debug, Default)]
pub struct Verdict {
    pub ticks: u64,
    /// The ticks the demo holds.
    pub of: u64,
    pub checked: u64,
    pub matched: Option<u64>,
    pub diverged: Option<Divergence>,
    pub taints: Vec<(u64, String)>,
    pub scenario: bool,
}
