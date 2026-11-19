//! One frame's resolved GPU times ([`GpuTimes`]) and how its ticks become them:
//! each timed pass charged its exclusive time (the module doc), and what of the
//! span the frame's pairs cover no row holds, split by whether it lies inside one
//! submit (untimed work, `gpu.other`) or between two (the wait for the CPU).

use super::GpuPass;

/// One query pair of a frame: the pass kind it adds into, and the queue submit
/// its passes go out in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Pair {
    pub pass: GpuPass,
    /// The renderer's submit sequence when the pair was taken
    /// (`gpu::ops::submit_seq`): pairs taken between the same two submits are
    /// recorded into one command buffer, so a gap between them is GPU work no pair
    /// holds, never a wait for the CPU. It is read when the pair is taken, not when
    /// its buffer is submitted, so it assumes no counted submit goes out while an
    /// encoder holding timed pairs is still open (another encoder submitted in
    /// between would tag this one's pairs a submit early). No site does that today.
    pub submit: u64,
}

/// Test-only: a pair in submit 0, for the tests that do not tell submits apart.
/// Live code takes its pairs from `GpuTimer::pair`, which tags the real submit.
#[cfg(test)]
impl From<GpuPass> for Pair {
    fn from(pass: GpuPass) -> Self {
        Self { pass, submit: 0 }
    }
}

/// One frame's GPU milliseconds per pass kind, in [`GpuPass::ALL`] order. A kind
/// with no pass that frame is left out. The rows, [`Self::other_ms`] and
/// [`Self::between_submits_ms`] partition the frame's timed span
/// ([`Self::span_ms`]), split in ticks before any is turned into ms.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GpuTimes {
    /// The drawn frame these times measure: the [`super::FrameCost::frames`] count
    /// that frame ended with. They arrive a frame or two later, so a caller pairing
    /// them with a frame pairs them by this, never by arrival.
    pub frame: u64,
    pub passes: Vec<(GpuPass, f64)>,
    /// Untimed GPU work, in ms: what of the span no row holds inside one submit,
    /// where the GPU never waits for the CPU. A pass left untimed shows here. It
    /// is GPU work, so it scales with the GPU's clock as the rows do.
    pub other_ms: f64,
    /// The gaps from one submit's last timed work to the next's first, in ms:
    /// mostly the GPU waiting for the CPU to record and submit, plus any untimed
    /// command at a submit's head or tail (SSR's copy of the opaque image, the
    /// Hi-Z counters' copies). A wait does not follow the GPU's clock, so this is
    /// read as measured, never scaled.
    pub between_submits_ms: f64,
}

impl GpuTimes {
    /// The frame's GPU time: every timed pass's, summed. Each pass is charged its
    /// exclusive busy time and no gap between passes, so this is time the GPU
    /// spent on timed work.
    pub fn total_ms(&self) -> f64 {
        self.passes.iter().map(|(_, ms)| ms).sum()
    }

    /// From the frame's earliest timed start to its latest timed end, in ms: the
    /// rows, [`Self::other_ms`] and [`Self::between_submits_ms`] together.
    pub fn span_ms(&self) -> f64 {
        self.total_ms() + self.other_ms + self.between_submits_ms
    }

    /// Sum resolved `ticks` — a begin/end pair per entry of `pairs`, in the order
    /// they were queued — into exclusive milliseconds per pass kind (see the
    /// module doc). `period_ns` is nanoseconds per tick. A pass that ends before the
    /// work queued ahead of it counts zero. A pair spread over several passes (its
    /// start written by the first, its end by the last) is one entry, charged once.
    /// A pair whose start reads 0 or past its end was not written whole: it is left
    /// out of its row, the span and the gaps, so it charges nothing and cannot
    /// stretch them to tick 0.
    pub(crate) fn from_ticks(ticks: &[u64], pairs: &[Pair], period_ns: f64) -> Self {
        let to_ms = |t: u64| t as f64 * period_ns / 1e6;
        let mut charged = [None::<u64>; GpuPass::ALL.len()];
        let mut done = None::<(u64, u64)>; // where the work queued so far ended, and its submit
        let mut span = None::<(u64, u64)>;
        let mut between = 0;
        for (ends, pair) in ticks.chunks_exact(2).zip(pairs) {
            let (start, end) = (ends[0], ends[1]);
            if start == 0 || start > end {
                continue;
            }
            let begin = done.map_or(start, |(d, _)| start.max(d));
            let slot = &mut charged[pair.pass as usize];
            *slot = Some(slot.unwrap_or(0) + end.saturating_sub(begin));
            if let Some((d, _)) = done.filter(|&(_, s)| s != pair.submit) {
                between += start.saturating_sub(d);
            }
            done = Some((done.map_or(end, |(d, _)| d.max(end)), pair.submit));
            span = Some(span.map_or((start, end), |(a, b)| (a.min(start), b.max(end))));
        }
        let busy: u64 = charged.iter().flatten().sum();
        let span = span.map_or(0, |(a, b)| b.saturating_sub(a));
        let passes = GpuPass::ALL
            .iter()
            .zip(charged)
            .filter_map(|(pass, ticks)| Some((*pass, to_ms(ticks?))))
            .collect();
        Self {
            frame: 0,
            passes,
            other_ms: to_ms(span.saturating_sub(busy).saturating_sub(between)),
            between_submits_ms: to_ms(between),
        }
    }
}

#[cfg(test)]
#[path = "times_tests.rs"]
mod tests;
