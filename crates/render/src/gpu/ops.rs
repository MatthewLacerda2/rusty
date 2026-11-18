//! crates/render/src/gpu/ops.rs — the renderer's counted GPU calls: every render
//! and compute pass it begins, every queue submit, bind group made and buffer write
//! goes through one of these, which counts it and forwards to wgpu unchanged.
//!
//! The counts are kept per thread and [`take`]n into a frame's
//! [`crate::RenderCounters::ops`]: `Renderer::render` and `render_capture_face`
//! drop what was counted before they start, and `finish_counters` takes what the
//! frame did. A call outside that window (setup, a load's warm-up, the GPU timer's
//! resolve after the counters close, a readback between frames) is counted into
//! nothing.
//!
//! The invariant is **one open render window at a time per thread**, not one
//! renderer per thread: two renderers on a thread (a test's headless pair, a
//! bake's renderer beside the editor's) share the one counter, so a frame of one
//! must not be drawn inside a frame of the other. A render nested inside a frame's
//! window on the same renderer goes through `Renderer::aside`, which keeps the
//! outer frame's counts ([`kept`]).
//!
//! `make rules`' `gpu-ops` rule holds that no file under `crates/render/src/` but
//! this one calls the raw wgpu methods outside test code.

use std::cell::Cell;

/// How many of each counted GPU call a frame made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OpCounts {
    /// Render passes begun.
    pub render_passes: u32,
    /// Compute passes begun.
    pub compute_passes: u32,
    /// Queue submits.
    pub submits: u32,
    /// Bind groups created.
    pub bind_groups_created: u32,
    /// `Queue::write_buffer` calls.
    pub buffer_writes: u32,
}

impl OpCounts {
    /// The counters under their `FrameStats` metric names.
    pub fn pairs(&self) -> [(&'static str, u64); 5] {
        [
            ("render_passes", self.render_passes.into()),
            ("compute_passes", self.compute_passes.into()),
            ("submits", self.submits.into()),
            ("bind_groups_created", self.bind_groups_created.into()),
            ("buffer_writes", self.buffer_writes.into()),
        ]
    }
}

thread_local! {
    static COUNTS: Cell<OpCounts> = const { Cell::new(OpCounts {
        render_passes: 0,
        compute_passes: 0,
        submits: 0,
        bind_groups_created: 0,
        buffer_writes: 0,
    }) };
}

/// What this thread counted since the last call, and start again from zero.
pub(crate) fn take() -> OpCounts {
    COUNTS.with(|c| c.take())
}

/// Run `f` and leave this thread's counts as they were before it: what `f` counts
/// (a render beside the frame, `Renderer::aside`) belongs to no frame, and what the
/// outer frame had counted survives `f`'s own [`take`]s.
pub(crate) fn kept<R>(f: impl FnOnce() -> R) -> R {
    let outer = take();
    let out = f();
    COUNTS.with(|c| c.set(outer));
    out
}

fn bump(add: impl FnOnce(&mut OpCounts)) {
    COUNTS.with(|c| {
        let mut n = c.get();
        add(&mut n);
        c.set(n);
    });
}

/// A command encoder's counted passes.
pub(crate) trait CountedEncoder {
    /// `begin_render_pass`, counted.
    fn counted_render_pass<'e>(
        &'e mut self,
        desc: &wgpu::RenderPassDescriptor<'_>,
    ) -> wgpu::RenderPass<'e>;
    /// `begin_compute_pass`, counted.
    fn counted_compute_pass<'e>(
        &'e mut self,
        desc: &wgpu::ComputePassDescriptor<'_>,
    ) -> wgpu::ComputePass<'e>;
}

impl CountedEncoder for wgpu::CommandEncoder {
    fn counted_render_pass<'e>(
        &'e mut self,
        desc: &wgpu::RenderPassDescriptor<'_>,
    ) -> wgpu::RenderPass<'e> {
        bump(|n| n.render_passes += 1);
        self.begin_render_pass(desc)
    }

    fn counted_compute_pass<'e>(
        &'e mut self,
        desc: &wgpu::ComputePassDescriptor<'_>,
    ) -> wgpu::ComputePass<'e> {
        bump(|n| n.compute_passes += 1);
        self.begin_compute_pass(desc)
    }
}

/// A queue's counted submits and buffer writes.
pub(crate) trait CountedQueue {
    /// `submit`, counted once whatever the number of command buffers.
    fn counted_submit<I: IntoIterator<Item = wgpu::CommandBuffer>>(
        &self,
        buffers: I,
    ) -> wgpu::SubmissionIndex;
    /// `write_buffer`, counted.
    fn counted_write_buffer(&self, buffer: &wgpu::Buffer, offset: wgpu::BufferAddress, data: &[u8]);
}

impl CountedQueue for wgpu::Queue {
    fn counted_submit<I: IntoIterator<Item = wgpu::CommandBuffer>>(
        &self,
        buffers: I,
    ) -> wgpu::SubmissionIndex {
        bump(|n| n.submits += 1);
        self.submit(buffers)
    }

    fn counted_write_buffer(
        &self,
        buffer: &wgpu::Buffer,
        offset: wgpu::BufferAddress,
        data: &[u8],
    ) {
        bump(|n| n.buffer_writes += 1);
        self.write_buffer(buffer, offset, data);
    }
}

/// A device's counted bind groups.
pub(crate) trait CountedDevice {
    /// `create_bind_group`, counted.
    fn counted_bind_group(&self, desc: &wgpu::BindGroupDescriptor<'_>) -> wgpu::BindGroup;
}

impl CountedDevice for wgpu::Device {
    fn counted_bind_group(&self, desc: &wgpu::BindGroupDescriptor<'_>) -> wgpu::BindGroup {
        bump(|n| n.bind_groups_created += 1);
        self.create_bind_group(desc)
    }
}
