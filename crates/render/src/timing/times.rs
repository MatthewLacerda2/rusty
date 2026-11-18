//! One frame's resolved GPU times ([`GpuTimes`]) and how its ticks become them:
//! each timed pass charged its exclusive time (the module doc), and the span the
//! frame's pairs cover, so what no row holds shows as `gpu.other`.

use super::GpuPass;

/// One query pair of a frame: the pass kind it adds into, and where its charge
/// opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pair {
    pub pass: GpuPass,
    /// Charged from where the work queued ahead of it ended, even when its first
    /// pass started later: the gap then holds a command recorded before that pass
    /// which no pass boundary can stamp (SSR's copy of the opaque image is a
    /// `copy_texture_to_texture`, not a pass). It also holds any wait for the CPU in
    /// that gap, so it is only for a pair right behind timed work.
    pub holds_gap: bool,
}

impl From<GpuPass> for Pair {
    fn from(pass: GpuPass) -> Self {
        Self {
            pass,
            holds_gap: false,
        }
    }
}

/// One frame's GPU milliseconds per pass kind, in [`GpuPass::ALL`] order. A kind
/// with no pass that frame is left out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GpuTimes {
    /// The drawn frame these times measure: the [`super::FrameCost::frames`] count
    /// that frame ended with. They arrive a frame or two later, so a caller pairing
    /// them with a frame pairs them by this, never by arrival.
    pub frame: u64,
    pub passes: Vec<(GpuPass, f64)>,
    /// From the frame's earliest timed start to its latest timed end, in ms: what
    /// the rows would add up to if every command of the frame were in one.
    pub span_ms: f64,
}

impl GpuTimes {
    /// The frame's GPU time: every timed pass's, summed.
    pub fn total_ms(&self) -> f64 {
        self.passes.iter().map(|(_, ms)| ms).sum()
    }

    /// What of [`Self::span_ms`] no row holds: work recorded outside every pair,
    /// and the GPU waiting for the CPU between the frame's submits. Near zero on a
    /// frame the GPU is the bottleneck of, once every pass is timed; a pass left
    /// untimed shows here.
    pub fn other_ms(&self) -> f64 {
        (self.span_ms - self.total_ms()).max(0.0)
    }

    /// Sum resolved `ticks` — a begin/end pair per entry of `pairs`, in the order
    /// they were queued — into exclusive milliseconds per pass kind (see the
    /// module doc). `period_ns` is nanoseconds per tick. A pass that ends before the
    /// work queued ahead of it counts zero; a pair that [`Pair::holds_gap`] opens
    /// where that work ended. A pair spread over several passes (its start written
    /// by the first, its end by the last) is one entry, charged once.
    pub fn from_ticks(ticks: &[u64], pairs: &[Pair], period_ns: f64) -> Self {
        let to_ms = |t: u64| t as f64 * period_ns / 1e6;
        let mut ms = [None::<f64>; GpuPass::ALL.len()];
        let mut done = None::<u64>; // where the work queued so far ended
        let mut span = None::<(u64, u64)>;
        for (ends, pair) in ticks.chunks_exact(2).zip(pairs) {
            let (start, end) = (ends[0], ends[1]);
            let begin = match done {
                Some(done) if pair.holds_gap => done,
                Some(done) => start.max(done),
                None => start,
            };
            done = Some(done.map_or(end, |d| d.max(end)));
            span = Some(span.map_or((start, end), |(a, b)| (a.min(start), b.max(end))));
            let slot = &mut ms[pair.pass as usize];
            *slot = Some(slot.unwrap_or(0.0) + to_ms(end.saturating_sub(begin)));
        }
        let passes = GpuPass::ALL
            .iter()
            .zip(ms)
            .filter_map(|(pass, ms)| Some((*pass, ms?)))
            .collect();
        let span_ms = span.map_or(0.0, |(a, b)| to_ms(b.saturating_sub(a)));
        Self {
            frame: 0,
            passes,
            span_ms,
        }
    }
}

#[cfg(test)]
#[path = "times_tests.rs"]
mod tests;
