use std::time::Instant;

use super::LoadSamples;
use rusty_app::LoadProbe;
use rusty_script::scripting::ConsoleLogs;

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
            return;
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
