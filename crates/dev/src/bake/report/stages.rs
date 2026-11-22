use std::time::{Duration, Instant};

use serde_json::{json, Value};

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
    pub fn start() -> Self {
        Self {
            last: Instant::now(),
            laps: Vec::new(),
        }
    }

    pub fn lap(&mut self, name: &str) {
        let now = Instant::now();
        self.laps.push((name.to_string(), now - self.last));
        self.last = now;
    }

    pub fn push(&mut self, name: &str, took: Duration) {
        self.laps.push((name.to_string(), took));
        self.last = Instant::now();
    }

    pub fn total(&self) -> Duration {
        self.laps.iter().map(|(_, d)| *d).sum()
    }

    pub fn to_json(&self) -> Value {
        let stage = |(name, took): &(String, Duration)| json!({ "name": name, "seconds": super::secs(*took) });
        Value::Array(self.laps.iter().map(stage).collect())
    }
}
