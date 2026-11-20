//! The staging memory texture uploads hold, bounded (#2372).
//!
//! `queue.write_texture` copies its bytes into a staging buffer that wgpu keeps
//! until the next submit and frees once the GPU has run it. That buffer is host
//! memory, counted in the process's resident memory on every GPU. An image upload
//! submits its own mip pass (`mips::MipGenerator::generate`), so its staging goes
//! as it is made. A compressed one draws nothing: its levels come whole from the
//! import cache. A frame or warm-up uploading a level's cached textures therefore
//! held every one's blocks in staging until the frame's own submit, so a project
//! with its import caches peaked higher than one decoding every PNG.
//!
//! [`Staging`] submits once the uploads since its last submit pass
//! [`STAGING_BUDGET`], after waiting for that last submit, so no more than two
//! budgets of uploads are in staging at once, whatever the scene's size.

use std::cell::{Cell, RefCell};

use crate::gpu::ops::CountedQueue as _;

/// The texture bytes written between two of [`Staging`]'s submits: two 2048² BC7
/// textures with their mips (5.6 MB each), or one 2048² RGBA8 base level.
pub const STAGING_BUDGET: u64 = 16 << 20;

/// The texture bytes written since the last submit made for them, and that
/// submit.
#[derive(Default)]
pub(crate) struct Staging {
    since: Cell<u64>,
    submitted: RefCell<Option<wgpu::SubmissionIndex>>,
}

impl Staging {
    /// Note `bytes` written to a texture; past the budget, wait for the last
    /// submit made here, then submit what is pending.
    pub(crate) fn wrote(&self, (device, queue): (&wgpu::Device, &wgpu::Queue), bytes: u64) {
        let since = self.since.get() + bytes;
        if since < STAGING_BUDGET {
            self.since.set(since);
            return;
        }
        self.since.set(0);
        if let Some(last) = self.submitted.take() {
            let wait = wgpu::PollType::Wait {
                submission_index: Some(last),
                timeout: None,
            };
            if let Err(e) = device.poll(wait) {
                log::warn!("[Render] waiting for a texture upload failed: {e}");
            }
        }
        *self.submitted.borrow_mut() = Some(queue.counted_submit([]));
    }
}
