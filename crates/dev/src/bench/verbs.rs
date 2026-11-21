use rusty_core::frame_stats::FrameStats;

use super::scenes::report::Row;
use super::Samples;
use crate::stats::verbs::{self, UPDATE_SYSTEM};

pub fn absent_is_zero(metric: &str) -> bool {
    metric.starts_with("script.")
}

pub const TOP: usize = 10;

#[derive(Default)]
pub(super) struct Window {
    seen: Vec<String>,
    frames: usize,
}

impl Window {
    pub(super) fn start() -> Self {
        verbs::take();
        Self::default()
    }

    pub(super) fn sample(&mut self, samples: &mut Samples, stats: &FrameStats, speed: Option<f64>) {
        let frame = verbs::take();
        let Some(speed) = speed.filter(|_| verbs::armed()) else {
            return;
        };
        let update = stats.tick_systems.iter().find(|(n, _)| *n == UPDATE_SYSTEM);
        self.push(samples, &frame, update.map_or(0.0, |(_, ms)| *ms), speed);
    }

    fn push(&mut self, samples: &mut Samples, frame: &verbs::Frame, update_ms: f64, speed: f64) {
        for verb in &frame.verbs {
            if !self.seen.contains(&verb.name) {
                for _ in 0..self.frames {
                    samples.push(&format!("script.{}", verb.name), 0.0);
                    samples.push(&format!("script.{}.calls", verb.name), 0.0);
                }
                self.seen.push(verb.name.clone());
            }
        }
        for name in &self.seen {
            let verb = frame.verbs.iter().find(|v| &v.name == name);
            let (ms, calls) = verb.map_or((0.0, 0), |v| (v.ms, v.calls));
            samples.push(&format!("script.{name}"), ms * speed);
            samples.push(&format!("script.{name}.calls"), f64::from(calls));
        }
        let calls: u32 = frame.verbs.iter().map(|v| v.calls).sum();
        samples.push("script.calls", f64::from(calls));
        let lua = (update_ms - frame.update_ms - frame.overhead_ms).max(0.0);
        samples.push("script.lua", lua * speed);
        samples.push("script.overhead", frame.overhead_ms * speed);
        self.frames += 1;
    }
}

pub fn keep_top(rows: &mut Vec<Row>) {
    let is_script = |r: &Row| r.metric.starts_with("script.");
    let (script, other): (Vec<Row>, Vec<Row>) = rows.drain(..).partition(is_script);
    *rows = other;
    let named = |m: &str| ["script.calls", "script.lua", "script.overhead"].contains(&m);
    rows.extend(script.iter().filter(|r| named(&r.metric)).cloned());
    let calls_of = |verb: &Row| {
        let metric = format!("{}.calls", verb.metric);
        script.iter().find(|r| r.metric == metric).cloned()
    };
    let mut verbs: Vec<&Row> = script
        .iter()
        .filter(|r| !named(&r.metric) && !r.metric.ends_with(".calls"))
        .collect();
    verbs.sort_by(|a, b| b.avg.total_cmp(&a.avg));
    let (mut rest, mut rest_calls) = (sum_row("script.other_verbs"), None);
    for (n, verb) in verbs.into_iter().enumerate() {
        if n < TOP {
            rows.push(verb.clone());
            rows.extend(calls_of(verb));
            continue;
        }
        add(&mut rest, verb);
        let total = rest_calls.get_or_insert_with(|| sum_row("script.other_verbs.calls"));
        if let Some(calls) = calls_of(verb) {
            add(total, &calls);
        }
    }
    if let Some(calls) = rest_calls {
        rows.push(rest);
        rows.push(calls);
    }
}

fn sum_row(metric: &str) -> Row {
    Row::value(metric.into(), 0.0)
}

fn add(sum: &mut Row, row: &Row) {
    sum.avg += row.avg;
    sum.p95 += row.p95;
    sum.lo += row.lo;
    sum.hi += row.hi;
}

#[cfg(test)]
#[path = "verbs_tests.rs"]
mod tests;
