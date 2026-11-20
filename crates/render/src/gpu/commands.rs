//! crates/render/src/gpu/commands.rs — one command encoder that several stages
//! record into and that goes out in one submit (#2353): a camera's whole frame,
//! the shadow sweep in the first camera's, the screen's UI in its last camera's.
//!
//! **What a stage may do while one records.** A `queue.write_buffer` (or
//! `write_texture`) lands at the next submit, *ahead of every command already
//! recorded*, so a stage never rewrites a buffer a command recorded before it reads:
//! it gives what changes its own buffer, or [`Commands::flush`]es first and says
//! why. Debug builds hold that for buffers (`gpu::ops::check_write`). A read-back
//! maps its buffer only after the copy into it is submitted: it registers the map
//! with [`Commands::after_send`], which runs it once the commands are sent.

use super::ops::{self, CountedQueue as _};

/// A map (or any step) that must wait for the recorded commands' submit.
type AfterSend = Box<dyn FnOnce()>;

/// An open command encoder, sent once ([`Commands::send`]; dropping it sends
/// it too, so a read-back it holds is never left unmapped).
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
        let outer = ops::begin_recording();
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
        ops::end_recording(self.outer);
        self.outer = ops::begin_recording();
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
            ops::forget_writes(); // they went out with it
        }
        for step in std::mem::take(&mut self.after) {
            step();
        }
    }
}

impl Drop for Commands {
    fn drop(&mut self) {
        self.submit_recorded();
        ops::end_recording(self.outer);
    }
}

fn encoder(device: &wgpu::Device, label: &'static str) -> wgpu::CommandEncoder {
    device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) })
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
