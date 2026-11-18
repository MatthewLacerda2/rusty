//! One frame's resolved GPU times ([`GpuTimes`]) and how its ticks become them:
//! each timed pass charged its exclusive time (the module doc), and the span the
//! frame's pairs cover, so what no row holds shows as `gpu.other`, split by
//! whether it lies inside one submit or between two.

use super::GpuPass;

/// One query pair of a frame: the pass kind it adds into, and the queue submit
/// its passes go out in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Pair {
    pub pass: GpuPass,
    /// The renderer's submit sequence when the pair was taken
    /// (`gpu::ops::submit_seq`): pairs taken between the same two submits are
    /// recorded into one command buffer, so a gap between them is GPU work no pair
    /// holds, never a wait for the CPU.
    pub submit: u64,
}

impl From<GpuPass> for Pair {
    fn from(pass: GpuPass) -> Self {
        Self { pass, submit: 0 }
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
    /// What of [`Self::other_ms`] lies in gaps from one submit's timed work to the
    /// next's, in ms: the GPU waiting for the CPU to record and submit, plus any
    /// untimed command at a submit's head or tail (SSR's copy of the opaque image,
    /// the Hi-Z counters' copies). The rest of `other_ms` is in gaps inside one
    /// submit: untimed GPU work, and nothing else.
    pub between_submits_ms: f64,
}

impl GpuTimes {
    /// The frame's GPU time: every timed pass's, summed. Each pass is charged its
    /// exclusive busy time and no gap between passes, so this is time the GPU
    /// spent on timed work.
    pub fn total_ms(&self) -> f64 {
        self.passes.iter().map(|(_, ms)| ms).sum()
    }

    /// What of [`Self::span_ms`] no row holds: work recorded outside every pair,
    /// and the GPU waiting for the CPU between the frame's submits
    /// ([`Self::between_submits_ms`] holds the second, with a submit's untimed
    /// edges). A pass left untimed shows here.
    pub fn other_ms(&self) -> f64 {
        (self.span_ms - self.total_ms()).max(0.0)
    }

    /// What of [`Self::other_ms`] is untimed GPU work inside a submit.
    pub fn within_submits_ms(&self) -> f64 {
        (self.other_ms() - self.between_submits_ms).max(0.0)
    }

    /// Sum resolved `ticks` — a begin/end pair per entry of `pairs`, in the order
    /// they were queued — into exclusive milliseconds per pass kind (see the
    /// module doc). `period_ns` is nanoseconds per tick. A pass that ends before the
    /// work queued ahead of it counts zero. A pair spread over several passes (its
    /// start written by the first, its end by the last) is one entry, charged once.
    /// A pair whose start reads 0 or past its end was not written whole: it is left
    /// out of the span and the gaps, so it cannot stretch them to tick 0.
    pub(crate) fn from_ticks(ticks: &[u64], pairs: &[Pair], period_ns: f64) -> Self {
        let to_ms = |t: u64| t as f64 * period_ns / 1e6;
        let mut ms = [None::<f64>; GpuPass::ALL.len()];
        let mut done = None::<(u64, u64)>; // where the work queued so far ended, and its submit
        let mut span = None::<(u64, u64)>;
        let mut between = 0;
        for (ends, pair) in ticks.chunks_exact(2).zip(pairs) {
            let (start, end) = (ends[0], ends[1]);
            let begin = done.map_or(start, |(d, _)| start.max(d));
            let slot = &mut ms[pair.pass as usize];
            *slot = Some(slot.unwrap_or(0.0) + to_ms(end.saturating_sub(begin)));
            if start == 0 || start > end {
                continue;
            }
            if let Some((d, _)) = done.filter(|&(_, s)| s != pair.submit) {
                between += start.saturating_sub(d);
            }
            done = Some((done.map_or(end, |(d, _)| d.max(end)), pair.submit));
            span = Some(span.map_or((start, end), |(a, b)| (a.min(start), b.max(end))));
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
            between_submits_ms: to_ms(between),
        }
    }
}

#[cfg(test)]
#[path = "times_tests.rs"]
mod tests;
