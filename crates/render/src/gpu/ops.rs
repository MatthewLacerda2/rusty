//! Counts are per thread: two renderers on one thread never nest frames, and a render inside
//! a frame goes through `Renderer::aside`.

use std::cell::Cell;
use std::panic::Location;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OpCounts {
    pub render_passes: u32,
    pub compute_passes: u32,
    pub submits: u32,
    pub bind_groups_created: u32,
    pub buffer_writes: u32,
}

impl OpCounts {
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

thread_local! {
    /// Counted submits on this thread, never reset: GPU timer pairs tagged with one count share
    /// a command buffer (`timing::times::Pair::submit`).
    static SUBMITS: Cell<u64> = const { Cell::new(0) };
}

pub(crate) fn submit_seq() -> u64 {
    SUBMITS.with(Cell::get)
}

pub(crate) fn take() -> OpCounts {
    COUNTS.with(|c| c.take())
}

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

pub(crate) trait CountedEncoder {
    fn counted_render_pass<'e>(
        &'e mut self,
        desc: &wgpu::RenderPassDescriptor<'_>,
    ) -> wgpu::RenderPass<'e>;
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

pub(crate) trait CountedQueue {
    fn counted_submit<I: IntoIterator<Item = wgpu::CommandBuffer>>(
        &self,
        buffers: I,
    ) -> wgpu::SubmissionIndex;
    #[track_caller]
    fn counted_write_buffer(&self, buffer: &wgpu::Buffer, offset: wgpu::BufferAddress, data: &[u8]);
    /// `staging` must be the renderer's for this queue, or its budget bounds nothing.
    fn counted_write_texture(
        &self,
        staging: &Staging,
        to: wgpu::TexelCopyTextureInfo<'_>,
        data: &[u8],
        layout: wgpu::TexelCopyBufferLayout,
        size: wgpu::Extent3d,
    );
}

impl CountedQueue for wgpu::Queue {
    fn counted_submit<I: IntoIterator<Item = wgpu::CommandBuffer>>(
        &self,
        buffers: I,
    ) -> wgpu::SubmissionIndex {
        bump(|n| n.submits += 1);
        SUBMITS.with(|s| s.set(s.get() + 1));
        self.submit(buffers)
    }

    #[track_caller]
    fn counted_write_buffer(
        &self,
        buffer: &wgpu::Buffer,
        offset: wgpu::BufferAddress,
        data: &[u8],
    ) {
        bump(|n| n.buffer_writes += 1);
        super::commands::check_write(
            buffer,
            offset..offset + data.len() as u64,
            Location::caller(),
        );
        self.write_buffer(buffer, offset, data);
    }

    fn counted_write_texture(
        &self,
        staging: &Staging,
        to: wgpu::TexelCopyTextureInfo<'_>,
        data: &[u8],
        layout: wgpu::TexelCopyBufferLayout,
        size: wgpu::Extent3d,
    ) {
        let staged = staging.make_room(self);
        self.write_texture(to, data, layout, size);
        staging.since.set(staged + data.len() as u64);
        staging.seq.set(submit_seq());
    }
}

pub(crate) type Writes<'a> = (&'a wgpu::Queue, &'a Staging);

/// Texture bytes between two of `Staging`'s own submits: two 2048² BC7 textures with
/// mips (5.6 MB each), or one 2048² RGBA8 base level.
/// A test hook: `dev`'s memory tests bound staging by it.
pub const STAGING_BUDGET: u64 = 16 << 20;

/// Bounds the host memory `write_texture` stages until a submit, submitting past the budget.
pub(crate) struct Staging {
    device: wgpu::Device,
    since: Cell<u64>,
    seq: Cell<u64>,
    submitted: Cell<Option<wgpu::SubmissionIndex>>,
}

impl Staging {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        Self {
            device: device.clone(),
            since: Cell::new(0),
            seq: Cell::new(submit_seq()),
            submitted: Cell::new(None),
        }
    }

    fn make_room(&self, queue: &wgpu::Queue) -> u64 {
        if submit_seq() != self.seq.get() {
            return 0;
        }
        let staged = self.since.get();
        if staged < STAGING_BUDGET {
            return staged;
        }
        if let Some(last) = self.submitted.take() {
            let wait = wgpu::PollType::Wait {
                submission_index: Some(last),
                timeout: None,
            };
            if let Err(e) = self.device.poll(wait) {
                log::warn!("[Render] waiting for a texture upload failed: {e}");
            }
        }
        self.submitted.set(Some(queue.counted_submit([])));
        0
    }
}

pub(crate) trait CountedDevice {
    fn counted_bind_group(&self, desc: &wgpu::BindGroupDescriptor<'_>) -> wgpu::BindGroup;
}

impl CountedDevice for wgpu::Device {
    fn counted_bind_group(&self, desc: &wgpu::BindGroupDescriptor<'_>) -> wgpu::BindGroup {
        bump(|n| n.bind_groups_created += 1);
        self.create_bind_group(desc)
    }
}
