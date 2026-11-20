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
//! panics on it ([`recording`], [`check_write`]).
//!
//! `make rules`' `gpu-ops` rule holds that no file under `crates/render/src/` but
//! this one calls the raw wgpu methods outside test code.

use std::cell::Cell;
use std::ops::Range;
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

thread_local! {
    /// The tag of the [`super::commands::Commands`] recording on this thread, if
    /// one is: every pair the GPU timer takes while it records goes out in its one
    /// submit, whatever other submit happens meanwhile (a buffer's growth copy).
    static RECORDING: Cell<Option<u64>> = const { Cell::new(None) };
    /// How many recordings this thread has begun, ever: each one's tag.
    static RECORDINGS: Cell<u64> = const { Cell::new(0) };
}

/// Tags a recording's pairs apart from any submit count.
const RECORDING_TAG: u64 = 1 << 63;

/// The submit the GPU timer's pair taken now goes out in: the open recording's
/// tag while one records, else how many counted submits this thread has made.
pub(crate) fn submit_seq() -> u64 {
    RECORDING
        .with(Cell::get)
        .unwrap_or_else(|| SUBMITS.with(Cell::get))
}

/// Start a recording on this thread, returning the one it nests in (restored by
/// [`end_recording`]). Its writes are checked from here to its end.
pub(crate) fn begin_recording() -> Option<u64> {
    let tag = RECORDING_TAG | RECORDINGS.with(|n| n.replace(n.get() + 1));
    let outer = RECORDING.with(|r| r.replace(Some(tag)));
    if outer.is_none() {
        forget_writes();
    }
    outer
}

/// End the recording [`begin_recording`] started, back to `outer`.
pub(crate) fn end_recording(outer: Option<u64>) {
    RECORDING.with(|r| r.set(outer));
    if outer.is_none() {
        forget_writes();
    }
}

/// Whether a recording is open on this thread.
pub(crate) fn recording() -> bool {
    RECORDING.with(Cell::get).is_some()
}

/// One buffer range written while a recording is open, and where.
#[cfg(debug_assertions)]
struct Written {
    buffer: wgpu::Buffer,
    range: Range<u64>,
    at: &'static Location<'static>,
}

#[cfg(debug_assertions)]
thread_local! {
    static WRITTEN: std::cell::RefCell<Vec<Written>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Forget the writes checked so far: the recording went out (`Commands::flush`).
pub(crate) fn forget_writes() {
    #[cfg(debug_assertions)]
    WRITTEN.with(|w| w.borrow_mut().clear());
}

/// Debug builds: panic when `range` of `buffer` was already written while the
/// open recording records (none open: nothing is checked). Release: nothing.
pub(crate) fn check_write(
    buffer: &wgpu::Buffer,
    range: Range<u64>,
    at: &'static Location<'static>,
) {
    #[cfg(debug_assertions)]
    if recording() {
        WRITTEN.with(|w| {
            let mut w = w.borrow_mut();
            let overlaps = |o: &&Written| {
                &o.buffer == buffer && o.range.start < range.end && range.start < o.range.end
            };
            if let Some(first) = w.iter().find(overlaps) {
                panic!("{}", twice_message(buffer.size(), &range, (first.at, at)));
            }
            w.push(Written {
                buffer: buffer.clone(),
                range,
                at,
            });
        });
    }
    #[cfg(not(debug_assertions))]
    let _ = (buffer, range, at);
}

/// [`check_write`]'s failure: which buffer, which writes, and what to do.
fn twice_message(size: u64, range: &Range<u64>, (first, again): (&Location, &Location)) -> String {
    format!(
        "a {size}-byte buffer's bytes {}..{} were written twice while one encoder \
         records (first at {first}, again at {again}): both writes land at its one \
         submit, so every command recorded between them reads the second. Give the \
         stage a buffer of its own, or send the open `Commands` first \
         (`gpu::commands::Commands::flush`)",
        range.start, range.end
    )
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
    /// writes ([`check_write`]).
    #[track_caller]
    fn counted_write_buffer(&self, buffer: &wgpu::Buffer, offset: wgpu::BufferAddress, data: &[u8]);
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
        check_write(
            buffer,
            offset..offset + data.len() as u64,
            Location::caller(),
        );
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
