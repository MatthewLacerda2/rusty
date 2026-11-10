//! A bake's wall time per stage, for its report's common block (#1980).

use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// The stages a bake ran, in order, each with its wall time. A stage is timed by
/// [`lap`](Self::lap) (since the last lap) or handed in already measured
/// ([`push`](Self::push): a worker's, or a job's spread over frames).
#[derive(Debug)]
pub struct Stages {
    last: Instant,
    laps: Vec<(String, Duration)>,
}

impl Default for Stages {
    fn default() -> Self {
        Self::start()
    }
}

impl Stages {
    /// Start timing now.
    pub fn start() -> Self {
        Self {
            last: Instant::now(),
            laps: Vec::new(),
        }
    }

    /// End the stage `name`: the time since the last lap (or the start).
    pub fn lap(&mut self, name: &str) {
        let now = Instant::now();
        self.laps.push((name.to_string(), now - self.last));
        self.last = now;
    }

    /// Add the stage `name`, measured elsewhere; the next lap starts now.
    pub fn push(&mut self, name: &str, took: Duration) {
        self.laps.push((name.to_string(), took));
        self.last = Instant::now();
    }

    /// Every stage's time added up: the bake's wall time.
    pub fn total(&self) -> Duration {
        self.laps.iter().map(|(_, d)| *d).sum()
    }

    pub fn to_json(&self) -> Value {
        let stage = |(name, took): &(String, Duration)| json!({ "name": name, "seconds": super::secs(*took) });
        Value::Array(self.laps.iter().map(stage).collect())
    }
}
