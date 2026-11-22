//! The probes that scale the bench's rows to a reference clock: `docs/testing.md`,
//! *GPU time and the GPU clock* and the section after it.

use std::hint::black_box;
use std::time::Instant;

use super::{Report, Samples};

/// Ms the GPU probe takes on an M4 at its top GPU clock, 1470 MHz: re-measure it when
/// `ClockProbe`'s load changes.
pub const REFERENCE_MS: f64 = 0.13;

pub fn speed(before: Option<f64>, after: Option<f64>) -> Option<f64> {
    let probe_ms = match (before, after) {
        (Some(a), Some(b)) => (a + b) / 2.0,
        (Some(ms), None) | (None, Some(ms)) => ms,
        (None, None) => return None,
    };
    Some(REFERENCE_MS / probe_ms)
}

/// Steps in the CPU probe's chain; each (a shift, an xor, a multiply: ~5 cycles) waits
/// on the last, so no core overlaps them.
const CPU_STEPS: u32 = 100_000;

/// Ms of [`CPU_STEPS`] steps at ~5 cycles and 4.4 GHz, derived, not measured: re-derive
/// it when the chain changes.
pub const CPU_REFERENCE_MS: f64 = 0.114;

pub fn cpu_probe_ms() -> f64 {
    let start = Instant::now();
    let mut x = black_box(0x9E37_79B9_7F4A_7C15_u64);
    for _ in 0..CPU_STEPS {
        x = (x ^ (x >> 29)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    }
    black_box(x);
    start.elapsed().as_secs_f64() * 1000.0
}

pub fn cpu_speed(before: f64, after: f64) -> Option<f64> {
    let probe_ms = (before + after) / 2.0;
    (probe_ms > 0.0 && probe_ms.is_finite()).then(|| CPU_REFERENCE_MS / probe_ms)
}

// Fractions past which the report warns: `docs/testing.md`, *CPU time and the CPU clock*.
pub const CLOCK_SPREAD: f64 = 0.15;
pub const CPU_SPREAD: f64 = 0.5;
pub const CLOCK_DRIFT: f64 = 0.10;

pub fn spread(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let at = |q: f64| sorted[((q * (sorted.len() - 1) as f64).round()) as usize];
    let median = at(0.5);
    if median == 0.0 {
        return 0.0;
    }
    (at(0.95) - at(0.05)) / median
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spread {
    Clock,
    Cpu,
}

#[derive(Clone, Copy, Debug)]
pub struct ClockRows {
    pub reference: &'static str,
    pub wall: &'static [&'static str],
}

pub const GAME_ROWS: ClockRows = ClockRows {
    reference: "cpu_ms",
    wall: &["frame_ms", "wall_ms"],
};

impl ClockRows {
    fn wall_list(self, and: &str) -> String {
        match self.wall.split_last() {
            None => String::new(),
            Some((last, [])) => (*last).into(),
            Some((last, rest)) => format!("{} {and} {last}", rest.join(", ")),
        }
    }
}

impl Spread {
    pub fn warning(self, by: f64, rows: ClockRows) -> String {
        let pct = by * 100.0;
        let reference = rows.reference;
        match self {
            Self::Clock => {
                let verb = if rows.wall.len() > 1 { "are" } else { "is" };
                format!(
                    "the CPU clock moved {pct:.0}% across the window: {} {verb} unreliable, \
                     read {reference}",
                    rows.wall_list("and")
                )
            }
            Self::Cpu => format!(
                "{reference} spread {pct:.0}% across the window: compare p95 with care, and take \
                 CPU numbers from a small-frame run (docs/testing.md)"
            ),
        }
    }
}

pub fn cpu_spreads(samples: &Samples) -> Vec<(Spread, f64)> {
    spreads_of(samples.values("cpu_clock_pct"), samples.values("cpu_ms"))
}

pub fn spreads_of(clock: &[f64], cpu: &[f64]) -> Vec<(Spread, f64)> {
    let (clock, cpu) = (spread(clock), spread(cpu));
    [
        (Spread::Clock, clock, CLOCK_SPREAD),
        (Spread::Cpu, cpu, CPU_SPREAD),
    ]
    .into_iter()
    .filter(|&(_, by, limit)| by > limit)
    .map(|(kind, by, _)| (kind, by))
    .collect()
}

pub fn cpu_warnings(samples: &Samples) -> Vec<String> {
    let spreads = cpu_spreads(samples);
    spreads
        .into_iter()
        .map(|(kind, by)| kind.warning(by, GAME_ROWS))
        .collect()
}

pub fn cpu_drift(now: &Report, previous: Option<&Report>) -> Option<String> {
    let clock = |r: &Report| {
        let row = r.metrics.iter().find(|(k, _)| k == "cpu_clock_pct");
        row.map(|(_, s)| s.avg)
    };
    clock_drift(clock(now)?, clock(previous?)?, GAME_ROWS)
}

pub fn clock_drift(now: f64, before: f64, rows: ClockRows) -> Option<String> {
    if now <= 0.0 || before <= 0.0 {
        return None;
    }
    let moved = (now - before) / before;
    (moved.abs() > CLOCK_DRIFT).then(|| {
        format!(
            "the CPU clock moved {:+.0}% since the last run: compare {}, not {}",
            moved * 100.0,
            rows.reference,
            rows.wall_list("or")
        )
    })
}

#[cfg(test)]
#[path = "clock_tests.rs"]
mod tests;
