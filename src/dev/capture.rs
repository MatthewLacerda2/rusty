//! src/dev/capture.rs — one renderer, many shots (#355 step 5).
//!
//! Every dev-layer capture — the screenshot, the asset preview — used to open with the
//! same line: build a whole `Renderer`, draw one frame, drop it. A `Renderer` is a wgpu
//! device, the full pipeline set, the shader registry, the shadow maps and every cached
//! mesh/texture, so "one shot, one device" meant a scenario taking six screenshots paid
//! for six devices, and a test comparing an effect on-vs-off paid for two. On Windows CI
//! that cost is not abstract: WARP is a software adapter whose textures live in system
//! RAM beside rustc, and unbounded concurrent renderers is exactly what exhausted it in
//! #366.
//!
//! That pattern existed because it *had* to. The renderer was a single-view machine:
//! size, depth buffer and post-FX chain were one shared copy, so a second consumer could
//! not borrow it without trampling the first's targets. Now that [`RenderView`] owns the
//! per-view state (#355 step 2) and the caches are namespaced per scene (step 3), a
//! renderer can serve any number of views of any number of scenes at any sizes — the
//! render path reads nothing size-dependent off the `Renderer` at all (`config` is
//! surface state, inert offscreen).
//!
//! [`CaptureHost`] is that fact made usable: hold one, take as many shots as you like.
//! It keeps the renderer *and* a view, both built on first use and reused after, so a
//! second shot at the same size allocates nothing — no device, no pipelines, no post-FX
//! shader compile. This is the step-5 proof the shared/per-view/per-scene boundaries are
//! drawn in the right places: if they weren't, the second shot would come out wrong.
//!
//! **No adapter is a skip, not a failure.** The Linux CI job has no GPU at all, so the
//! host reports `None` and its callers return `Ok(false)` — the same contract
//! `Renderer::new_headless` has always had, kept in one place. The probe is attempted
//! once; a box with no adapter does not retry per shot.
//!
//! Allowed deps: render (headless path).

use std::path::Path;

use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};

/// A renderer + view held across captures, so N shots cost one device.
///
/// Build one, hand it to as many capture calls as you like, and drop it when the batch
/// is done — the renderer holds a slot in the headless budget
/// (`render::setup::budget`) for exactly as long as the host lives, so a long-lived
/// host is a long-held slot. Own one per batch of work (a harness run, a test), not one
/// per process.
#[derive(Default)]
pub struct CaptureHost {
    /// The shared renderer, built on the first capture. `None` until then, and
    /// permanently `None` on a box with no adapter (see `probed`).
    renderer: Option<Renderer>,
    /// The view every shot draws through, resized in place when a shot asks for a size
    /// the last one didn't use. Owning its colour target (`RenderView::offscreen`) is
    /// what lets the readback copy from the very texture the pass drew.
    view: Option<RenderView>,
    /// Whether an adapter request has already been made. Stops a GPU-less box from
    /// paying for a failed adapter probe on every single shot.
    probed: bool,
}

impl CaptureHost {
    /// An empty host. Nothing is allocated until the first capture asks for a frame.
    pub fn new() -> Self {
        Self::default()
    }

    /// The renderer and a view sized to `width` x `height`, building either on first
    /// use and reusing both afterwards.
    ///
    /// `None` means this machine has no GPU or software adapter: **skip, don't fail**.
    /// Callers turn that into their own graceful outcome (`Ok(false)`), matching the
    /// contract every headless path in the engine already has.
    pub fn frame(&mut self, width: u32, height: u32) -> Option<(&mut Renderer, &mut RenderView)> {
        let (width, height) = (width.max(1), height.max(1));
        if !self.probed {
            self.probed = true;
            // Born at the first shot's size purely so a lone capture is identical to the
            // unshared path; nothing in the render path reads `config` offscreen, which
            // is precisely why later shots may ask for any other size.
            self.renderer = pollster::block_on(Renderer::new_headless(width, height));
        }
        let renderer = self.renderer.as_mut()?;
        // Create-or-resize, the same guard the editor viewport uses: a repeat shot at
        // the same size is a no-op rather than a reallocation.
        RenderView::ensure_offscreen(
            &mut self.view,
            &renderer.device,
            OFFSCREEN_FORMAT,
            width,
            height,
            renderer.quality.bloom_divisor(),
        );
        Some((renderer, self.view.as_mut()?))
    }

    /// Copy the view's colour target back to the CPU and encode it as a PNG at `path`.
    ///
    /// Call after a `render` into the frame's target view. Errors only on an actual
    /// I/O / encode failure — by this point a frame has definitely been produced.
    pub fn write_png(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let (Some(renderer), Some(view)) = (self.renderer.as_ref(), self.view.as_ref()) else {
            return Err("capture host has no rendered frame to write".to_string());
        };
        let target = view
            .color_target()
            .ok_or_else(|| "capture view owns no colour target".to_string())?;
        let size = view.size();
        let pixels = readback::read_texture_rgba8(
            &renderer.device,
            &renderer.queue,
            target,
            size.width,
            size.height,
        );
        readback::write_png(path, size.width, size.height, &pixels)
    }
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod capture_tests;
