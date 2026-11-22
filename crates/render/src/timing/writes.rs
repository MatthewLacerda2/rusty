use super::{GpuPart, GpuPass, GpuTimer, Pair, PAIRS};

type RenderWrites<'a> = Option<wgpu::RenderPassTimestampWrites<'a>>;
type ComputeWrites<'a> = Option<wgpu::ComputePassTimestampWrites<'a>>;

type Indices = (Option<u32>, Option<u32>);

impl GpuTimer {
    pub(crate) fn writes(&self, pass: GpuPass) -> RenderWrites<'_> {
        let (set, at) = self.pair(pass, None)?;
        Some(render(set, whole(at)))
    }

    pub(crate) fn compute_writes(&self, pass: GpuPass) -> ComputeWrites<'_> {
        let (set, at) = self.pair(pass, None)?;
        Some(compute(set, whole(at)))
    }

    pub(crate) fn part_writes(&self, part: GpuPart) -> RenderWrites<'_> {
        let (set, at) = self.pair(part.pass(), Some(part))?;
        Some(render(set, whole(at)))
    }

    pub(crate) fn part_compute_writes(&self, part: GpuPart) -> ComputeWrites<'_> {
        let (set, at) = self.pair(part.pass(), Some(part))?;
        Some(compute(set, whole(at)))
    }

    /// One pair across several passes (the first opens, the last closes): all must go out in one
    /// submit.
    pub(crate) fn render_span(&self, pass: GpuPass) -> (RenderWrites<'_>, RenderWrites<'_>) {
        let Some((set, at)) = self.pair(pass, None) else {
            return (None, None);
        };
        let [open, close] = halves(at);
        (Some(render(set, open)), Some(render(set, close)))
    }

    fn pair(&self, pass: GpuPass, part: Option<GpuPart>) -> Option<(&wgpu::QuerySet, u32)> {
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
        let submit = crate::gpu::commands::pair_tag();
        frame.push(Pair { pass, part, submit });
        Some((&q.set, at))
    }
}

fn whole(at: u32) -> Indices {
    (Some(at * 2), Some(at * 2 + 1))
}

fn halves(at: u32) -> [Indices; 2] {
    [(Some(at * 2), None), (None, Some(at * 2 + 1))]
}

fn render(
    query_set: &wgpu::QuerySet,
    (begin, end): Indices,
) -> wgpu::RenderPassTimestampWrites<'_> {
    wgpu::RenderPassTimestampWrites {
        query_set,
        beginning_of_pass_write_index: begin,
        end_of_pass_write_index: end,
    }
}

fn compute(
    query_set: &wgpu::QuerySet,
    (begin, end): Indices,
) -> wgpu::ComputePassTimestampWrites<'_> {
    wgpu::ComputePassTimestampWrites {
        query_set,
        beginning_of_pass_write_index: begin,
        end_of_pass_write_index: end,
    }
}
