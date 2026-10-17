//! How fast the GPU is clocked right now, measured with a fixed load (#862).
//!
//! The harness steps the sim on the CPU, then renders and waits for the GPU, so
//! the GPU idles part of every frame. Apple silicon's GPU governor reads that idle
//! time and moves the clock (338-1470 MHz on an M4, re-decided many times a
//! second), and a lighter frame idles longer and runs slower. GPU milliseconds
//! then measure the governor as much as the work.
//!
//! [`ClockProbe`](crate::render::ClockProbe) (in `render::timing` since #989) times
//! the same small arithmetic dispatch right before and right after every frame. Its
//! duration follows the clock and nothing else, so scaling the frame's GPU time by
//! [`speed`] — how fast the probe ran against [`REFERENCE_MS`] — divides the clock
//! out: the result is what the frame would have taken at the reference clock.

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

#[cfg(test)]
#[path = "clock_tests.rs"]
mod tests;
