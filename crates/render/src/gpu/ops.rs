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
//! While a [`super::commands::Commands`] records (a camera's frame, #2353), a
//! `write_buffer` lands at its one submit, ahead of every command it recorded, so
//! a range written twice in that window leaves the commands recorded between the
//! two writes reading the second. In debug builds [`CountedQueue::counted_write_buffer`]
//! panics on it (`gpu::commands::check_write`).
//!
//! A texture write is the one call counted into something else: its bytes go to
//! the renderer's [`Staging`], which bounds what wgpu holds of them until a
//! submit (#2372).
//!
//! `make rules`' `gpu-ops` rule holds that no file under `crates/render/src/` but
//! this one calls the raw wgpu methods outside test code.

use std::cell::Cell;
use std::panic::Location;

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

thread_local! {
    /// Every counted submit this thread made, never reset: the GPU timer tags
    /// each pair with it, so pairs between the same two submits are known to
    /// share a command buffer (`timing::times::Pair::submit`).
    static SUBMITS: Cell<u64> = const { Cell::new(0) };
}

/// How many counted submits this thread has made, ever.
pub(crate) fn submit_seq() -> u64 {
    SUBMITS.with(Cell::get)
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
    /// `write_buffer`, counted, and checked against the open recording's other
    /// writes (`gpu::commands::check_write`).
    #[track_caller]
    fn counted_write_buffer(&self, buffer: &wgpu::Buffer, offset: wgpu::BufferAddress, data: &[u8]);
    /// `write_texture`, its bytes counted into `staging`, the renderer's for
    /// this queue ([`Staging`]).
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

/// A queue and its [`Staging`]: what a counted texture write needs.
pub(crate) type Writes<'a> = (&'a wgpu::Queue, &'a Staging);

/// The texture bytes written between two of `Staging`'s own submits: two 2048²
/// BC7 textures with their mips (5.6 MB each), or one 2048² RGBA8 base level.
pub const STAGING_BUDGET: u64 = 16 << 20;

/// What one queue's texture writes hold in staging, bounded.
///
/// `queue.write_texture` copies its bytes into a staging buffer that wgpu keeps
/// until the next submit and frees once the GPU has run it. That buffer is host
/// memory, counted in the process's resident memory on every GPU. A write that a
/// submit follows soon (an image upload with mips, whose mip pass submits) holds
/// it briefly; one that nothing submits after (a compressed texture's levels, a
/// lightmap's pages, a single-level image) holds it until the frame's own submit,
/// so a load of many piled up until then (#2372).
///
/// The bytes written since the last submit on this thread are what is still
/// staged: any counted submit (a mip pass, a frame) takes them with it. Before a
/// write that would find [`STAGING_BUDGET`] or more staged, `Staging` waits for
/// its own previous submit and submits what is pending. So its own submits keep
/// at most two batches, each the budget plus the one write that crossed it, in
/// staging at once, however many textures a scene has; a write a submit already
/// follows pays no wait and no submit. A submit made elsewhere is not waited
/// for. Like the frame's counts, the submits are counted per thread, so two
/// renderers on one thread (a test's pair, a bake beside the editor) reset each
/// other's count.
pub(crate) struct Staging {
    device: wgpu::Device,
    since: Cell<u64>,
    /// [`submit_seq`] when `since` was last counted: a submit since took it.
    seq: Cell<u64>,
    submitted: Cell<Option<wgpu::SubmissionIndex>>,
}

impl Staging {
    /// An empty count for the queue of `device`.
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        Self {
            device: device.clone(),
            since: Cell::new(0),
            seq: Cell::new(submit_seq()),
            submitted: Cell::new(None),
        }
    }

    /// The bytes still staged before a write: none when a submit took them, and
    /// none once at the budget, after waiting for the last submit made here and
    /// submitting what is pending on `queue`.
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
