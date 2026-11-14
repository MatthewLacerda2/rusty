//! How fast the GPU and the CPU are clocked right now, each measured with a fixed
//! load (#862, #1719).
//!
//! The harness steps the sim on the CPU, then renders and waits for the GPU, so
//! the GPU idles part of every frame. Apple silicon's GPU governor reads that idle
//! time and moves the clock (338-1470 MHz on an M4, re-decided many times a
//! second), and a lighter frame idles longer and runs slower. GPU milliseconds
//! then measure the governor as much as the work.
//!
//! [`ClockProbe`](rusty_render::ClockProbe) (in `render::timing` since #989) times
//! the same small arithmetic dispatch right before and right after every frame. Its
//! duration follows the clock and nothing else, so scaling the frame's GPU time by
//! [`speed`] — how fast the probe ran against [`REFERENCE_MS`] — divides the clock
//! out: the result is what the frame would have taken at the reference clock.
//!
//! The CPU has the same problem from the other side (#1719): on Apple silicon the
//! GPU and CPU share one power budget, so a big frame that keeps the GPU at full
//! clock pulls the CPU's clock down, and the same sim read 1.4 to 4.9 ms a tick
//! across runs at a Retina-size frame. [`cpu_probe_ms`] times a fixed chain of
//! integer steps on the CPU right before and right after the sim steps;
//! [`cpu_speed`] turns that into the same kind of ratio against
//! [`CPU_REFERENCE_MS`]. It needs no OS frequency API, so it works on macOS and
//! Linux alike.

use std::hint::black_box;
use std::time::Instant;

use super::{Report, Samples};

/// The probe's time at the reference clock: what it takes on an Apple M4 at its
/// top GPU clock (1470 MHz), so on that machine the bench's GPU rows read as ms
/// at full speed. Calibrated against the probe's fixed load: changing that load
/// means re-measuring this. On any other GPU they are that GPU's own consistent unit.
pub const REFERENCE_MS: f64 = 0.13;

/// How fast the GPU ran over a frame against the reference clock (1.0 = the
/// reference, 0.5 = half as fast), from the probe timed before and after it.
/// `None` when neither probe read.
pub fn speed(before: Option<f64>, after: Option<f64>) -> Option<f64> {
    let probe_ms = match (before, after) {
        (Some(a), Some(b)) => (a + b) / 2.0,
        (Some(ms), None) | (None, Some(ms)) => ms,
        (None, None) => return None,
    };
    Some(REFERENCE_MS / probe_ms)
}

/// Steps in the CPU probe's chain. Each step depends on the one before (a shift,
/// an xor and a multiply: about five cycles on x86_64 and Apple silicon alike), so
/// no core can overlap them and the time is steps × cycles ÷ clock.
const CPU_STEPS: u32 = 100_000;

/// The CPU probe's time at the reference clock: [`CPU_STEPS`] steps of ~5 cycles
/// at 4.4 GHz, an M4 performance core's top clock. Derived, not measured on an
/// M4: on any machine the CPU rows are that machine's own consistent unit, and
/// `cpu_clock_pct` may read above 100. Changing the chain means re-deriving this.
pub const CPU_REFERENCE_MS: f64 = 0.114;

/// Time the CPU probe once, in ms: the fixed chain of [`CPU_STEPS`] steps.
pub fn cpu_probe_ms() -> f64 {
    let start = Instant::now();
    let mut x = black_box(0x9E37_79B9_7F4A_7C15_u64);
    for _ in 0..CPU_STEPS {
        x = (x ^ (x >> 29)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    }
    black_box(x);
    start.elapsed().as_secs_f64() * 1000.0
}

/// How fast the CPU ran over a sim step against the reference clock, from the
/// probe timed before and after it. A probe that read zero (a clock too coarse to
/// see it) is not a reading: `None` then.
pub fn cpu_speed(before: f64, after: f64) -> Option<f64> {
    let probe_ms = (before + after) / 2.0;
    (probe_ms > 0.0 && probe_ms.is_finite()).then(|| CPU_REFERENCE_MS / probe_ms)
}

/// How far `cpu_clock_pct` may spread across one window (p5 to p95, over its
/// median) before the report warns: past it, the clock changed under the run.
pub const CLOCK_SPREAD: f64 = 0.15;
/// How far `cpu_ms` may spread across one window before the report warns.
pub const CPU_SPREAD: f64 = 0.5;
/// How far the average CPU clock may move between two runs before the report
/// warns that their raw CPU rows do not compare.
pub const CLOCK_DRIFT: f64 = 0.10;

/// The p5-to-p95 range of `values` over their median (0 for fewer than two, or
/// a median of zero).
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

/// The warnings one window's CPU samples earn: a clock that moved during it,
/// and CPU times that spread past [`CPU_SPREAD`].
pub fn cpu_warnings(samples: &Samples) -> Vec<String> {
    let mut out = Vec::new();
    let clock = spread(samples.values("cpu_clock_pct"));
    if clock > CLOCK_SPREAD {
        out.push(format!(
            "the CPU clock moved {:.0}% across the window: frame_ms is unreliable, read cpu_ms",
            clock * 100.0
        ));
    }
    let cpu = spread(samples.values("cpu_ms"));
    if cpu > CPU_SPREAD {
        out.push(format!(
            "cpu_ms spread {:.0}% across the window: compare p95 with care, and take CPU \
             numbers from a small-frame run (docs/testing.md)",
            cpu * 100.0
        ));
    }
    out
}

/// A warning when the average CPU clock moved past [`CLOCK_DRIFT`] since the
/// `previous` run: their `frame_ms` rows then measure the clock, not the work.
pub fn cpu_drift(now: &Report, previous: Option<&Report>) -> Option<String> {
    let clock = |r: &Report| {
        let row = r.metrics.iter().find(|(k, _)| k == "cpu_clock_pct");
        row.map(|(_, s)| s.avg).filter(|&avg| avg > 0.0)
    };
    let (now, before) = (clock(now)?, clock(previous?)?);
    let moved = (now - before) / before;
    (moved.abs() > CLOCK_DRIFT).then(|| {
        format!(
            "the CPU clock moved {:+.0}% since the last run: compare cpu_ms, not frame_ms",
            moved * 100.0
        )
    })
}

#[cfg(test)]
#[path = "clock_tests.rs"]
mod tests;
