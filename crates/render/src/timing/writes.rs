//! The `timestamp_writes` the timer hands a pass: a whole pair, a pair that holds
//! the gap before it ([`Pair::holds_gap`]), or one pair spread over several passes.

use super::{GpuPass, GpuTimer, Pair, PAIRS};

type RenderWrites<'a> = Option<wgpu::RenderPassTimestampWrites<'a>>;
type ComputeWrites<'a> = Option<wgpu::ComputePassTimestampWrites<'a>>;

impl GpuTimer {
    /// The `timestamp_writes` for one render pass of kind `pass`; `None` when the
    /// timer is disabled, no frame is open, or the frame ran out of query pairs.
    pub(crate) fn writes(&self, pass: GpuPass) -> RenderWrites<'_> {
        let (query_set, at) = self.pair(pass.into())?;
        Some(wgpu::RenderPassTimestampWrites {
            query_set,
            beginning_of_pass_write_index: Some(at * 2),
            end_of_pass_write_index: Some(at * 2 + 1),
        })
    }

    /// [`Self::writes`] for a render pass whose pair is charged from where the
    /// work queued ahead of it ended ([`Pair::holds_gap`]): the untimed command
    /// recorded just before it is charged to `pass`.
    pub(crate) fn writes_holding_gap(&self, pass: GpuPass) -> RenderWrites<'_> {
        let holds_gap = true;
        let (query_set, at) = self.pair(Pair { pass, holds_gap })?;
        Some(wgpu::RenderPassTimestampWrites {
            query_set,
            beginning_of_pass_write_index: Some(at * 2),
            end_of_pass_write_index: Some(at * 2 + 1),
        })
    }

    /// [`Self::writes`] for a compute pass.
    pub(crate) fn compute_writes(&self, pass: GpuPass) -> ComputeWrites<'_> {
        let (query_set, at) = self.pair(pass.into())?;
        Some(wgpu::ComputePassTimestampWrites {
            query_set,
            beginning_of_pass_write_index: Some(at * 2),
            end_of_pass_write_index: Some(at * 2 + 1),
        })
    }

    /// One pair of kind `pass` spread over several render passes: the first pass
    /// takes the opening half (its start), the last the closing half (its end), so
    /// every pass recorded between them is charged to `pass` once.
    pub(crate) fn render_span(&self, pass: GpuPass) -> (RenderWrites<'_>, RenderWrites<'_>) {
        let Some((query_set, at)) = self.pair(pass.into()) else {
            return (None, None);
        };
        let half = |begin, end| wgpu::RenderPassTimestampWrites {
            query_set,
            beginning_of_pass_write_index: begin,
            end_of_pass_write_index: end,
        };
        (
            Some(half(Some(at * 2), None)),
            Some(half(None, Some(at * 2 + 1))),
        )
    }

    /// [`Self::render_span`] over compute passes.
    pub(crate) fn compute_span(&self, pass: GpuPass) -> (ComputeWrites<'_>, ComputeWrites<'_>) {
        let Some((query_set, at)) = self.pair(pass.into()) else {
            return (None, None);
        };
        let half = |begin, end| wgpu::ComputePassTimestampWrites {
            query_set,
            beginning_of_pass_write_index: begin,
            end_of_pass_write_index: end,
        };
        (
            Some(half(Some(at * 2), None)),
            Some(half(None, Some(at * 2 + 1))),
        )
    }

    /// The next query pair, taken for `pair`.
    fn pair(&self, pair: Pair) -> Option<(&wgpu::QuerySet, u32)> {
        #[cfg(test)]
        self.asked.set(self.asked.get() + 1);
        let q = self.queries.as_ref().filter(|q| q.open.get())?;
        let mut frame = q.frame.borrow_mut();
        let at = frame.len() as u32;
        if at >= PAIRS {
            if !q.overflowed.replace(true) {
                log::warn!("[GpuTimer] a frame ran past {PAIRS} timed passes; the rest go untimed");
            }
            return None;
        }
        frame.push(pair);
        Some((&q.set, at))
    }
}
