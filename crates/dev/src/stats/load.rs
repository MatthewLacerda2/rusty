//! crates/dev/src/stats/load.rs — the load timer (#2123): a play-mode scene load,
//! step by step, into the [`TimingProbe`](super::TimingProbe)'s stats.

use std::time::Instant;

use super::LoadSamples;
use rusty_app::LoadProbe;
use rusty_script::scripting::ConsoleLogs;

/// Times each step of a play-mode scene load (#2123) and, once it swapped in,
/// reports them: one console line at once, and `load_<step>_ms` plus `load_ms`
/// (their sum) into `loads`, which the [`TimingProbe`] records at the frame's end
/// into the stats `Debug.Stats()` reads, so a series spans every load of the session.
pub struct LoadTimer {
    loads: LoadSamples,
    steps: Vec<(&'static str, f64)>,
}

impl LoadTimer {
    pub fn new(loads: LoadSamples) -> Self {
        let steps = Vec::new();
        Self { loads, steps }
    }
}

impl LoadProbe for LoadTimer {
    fn step(&mut self, name: &'static str, step: &mut dyn FnMut()) {
        let start = Instant::now();
        step();
        self.steps
            .push((name, start.elapsed().as_secs_f64() * 1000.0));
    }

    fn finish(&mut self, loaded: Option<&str>, console: &mut ConsoleLogs) {
        let steps = std::mem::take(&mut self.steps);
        let Some(path) = loaded else {
            return; // a failed load swapped nothing in: nothing to compare against
        };
        let total: f64 = steps.iter().map(|(_, ms)| ms).sum();
        let mut loads = self.loads.borrow_mut();
        let parts: Vec<String> = steps
            .iter()
            .map(|(name, ms)| {
                loads.push((format!("load_{name}_ms"), *ms));
                format!("{name} {ms:.1}")
            })
            .collect();
        loads.push(("load_ms".to_string(), total));
        console.info(format!(
            "Scene load {path}: {total:.1} ms ({})",
            parts.join(", ")
        ));
    }
}
