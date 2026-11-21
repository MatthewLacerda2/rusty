//! crates/render/src/gpu/commands.rs — one command encoder that several stages
//! record into and that goes out in one submit (#2353): a camera's whole frame,
//! the shadow sweep in the first camera's, the screen's UI in its last camera's.
//!
//! **What a stage may do while one records.** A `queue.write_buffer` (or
//! `write_texture`) lands at the next submit, *ahead of every command already
//! recorded*, so a stage never rewrites a buffer a command recorded before it reads:
//! it gives what changes its own buffer, or [`Commands::flush`]es first and says
//! why. Debug builds catch one half of that: a byte range of a buffer written
//! **twice** while a recording is open panics ([`check_write`]), naming both
//! writes. A single write after a command that reads the buffer was recorded is
//! *not* checked (nothing here sees what a pass reads): keeping that one is the
//! stage author's. A read-back maps its buffer only after the copy into it is
//! submitted: it registers the map with [`Commands::after_send`], which runs it
//! once the commands are sent.

use super::ops::{self, CountedQueue as _};
use std::cell::Cell;
use std::ops::Range;
use std::panic::Location;

/// A map (or any step) that must wait for the recorded commands' submit.
type AfterSend = Box<dyn FnOnce()>;

/// An open command encoder, sent once ([`Commands::send`]; dropping it sends
/// it too, so a read-back it holds is never left unmapped, except while the
/// thread panics: a half-recorded frame is dropped unsent).
pub(crate) struct Commands {
    encoder: Option<wgpu::CommandEncoder>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    label: &'static str,
    after: Vec<AfterSend>,
    /// The recording this one nests in (`gpu::ops::begin_recording`).
    outer: Option<u64>,
}

impl Commands {
    /// Open a recording labelled `label`.
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue, label: &'static str) -> Self {
        let outer = begin_recording();
        Self {
            encoder: Some(encoder(device, label)),
            device: device.clone(),
            queue: queue.clone(),
            label,
            after: Vec::new(),
            outer,
        }
    }

    /// The encoder the stages record into.
    pub(crate) fn encoder(&mut self) -> &mut wgpu::CommandEncoder {
        self.encoder
            .as_mut()
            .expect("an open recording has its encoder")
    }

    /// Run `step` once these commands are submitted: a read-back's `map_async`,
    /// which wgpu refuses while the copy into its buffer waits unsubmitted.
    pub(crate) fn after_send(&mut self, step: impl FnOnce() + 'static) {
        self.after.push(Box::new(step));
    }

    /// Submit what was recorded so far and keep recording into a fresh encoder,
    /// a new recording: at a camera's end, or for a stage that must rewrite a
    /// buffer an earlier command reads.
    pub(crate) fn flush(&mut self) {
        self.submit_recorded();
        end_recording(self.outer);
        self.outer = begin_recording();
        self.encoder = Some(encoder(&self.device, self.label));
    }

    /// Submit the recorded commands, then run what waited for them.
    pub(crate) fn send(mut self) {
        self.submit_recorded();
    }

    fn submit_recorded(&mut self) {
        let Some(encoder) = self.encoder.take() else {
            return;
        };
        self.queue.counted_submit(std::iter::once(encoder.finish()));
        if self.outer.is_none() {
            forget_writes(); // they went out with it
        }
        for step in std::mem::take(&mut self.after) {
            step();
        }
    }
}

impl Drop for Commands {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            self.submit_recorded();
        }
        end_recording(self.outer);
    }
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

/// The submit a GPU timer pair taken now goes out in: the open recording's tag
/// while one records, else [`ops::submit_seq`].
pub(crate) fn pair_tag() -> u64 {
    RECORDING.with(Cell::get).unwrap_or_else(ops::submit_seq)
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
#[cfg(any(debug_assertions, test))]
pub(crate) fn recording() -> bool {
    RECORDING.with(Cell::get).is_some()
}

/// Where each range of a buffer was written while a recording is open.
#[cfg(debug_assertions)]
type Written = Vec<(Range<u64>, &'static Location<'static>)>;

#[cfg(debug_assertions)]
thread_local! {
    /// The open recording's writes, keyed by buffer: a check walks only its own
    /// buffer's ranges.
    static WRITTEN: std::cell::RefCell<std::collections::HashMap<wgpu::Buffer, Written>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Forget the writes checked so far: the recording went out ([`Commands::flush`]).
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
            let ranges = w.entry(buffer.clone()).or_default();
            let overlaps = |(o, _): &&(Range<u64>, _)| o.start < range.end && range.start < o.end;
            if let Some((_, first)) = ranges.iter().find(overlaps) {
                let message = twice_message(buffer.size(), &range, (first, at));
                drop(w);
                panic!("{message}");
            }
            ranges.push((range, at));
        });
    }
    #[cfg(not(debug_assertions))]
    let _ = (buffer, range, at);
}

/// [`check_write`]'s failure: which buffer, which writes, and what to do.
#[cfg(debug_assertions)]
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

fn encoder(device: &wgpu::Device, label: &'static str) -> wgpu::CommandEncoder {
    device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) })
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
