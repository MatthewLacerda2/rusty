//! src/dev/screenshot.rs — Offscreen render -> PNG (the agent's eyes)
//!
//! The ONLY place the dev layer touches the renderer. Builds a wgpu device with NO
//! window surface (`Renderer::new_headless`), renders ONE frame into an offscreen
//! colour texture via the same `Renderer::render` the editor uses, then hands the
//! target to `render::readback` for the copy-back + PNG encode. Lets an agent
//! literally SEE a frame and judge lighting / SSR / shadows against the CS1.6 ->
//! FEAR -> Trepang2 visual bar.
//!
//! Runtime caveat: needs a GPU or software adapter (e.g. lavapipe) in the
//! container. When none is available, `capture`/`capture_world` return
//! `Ok(false)` after a clear log — they NEVER panic, and the rest of the dev layer
//! (Step / StepUntil / state) needs no GPU at all.
//!
//! Allowed deps: render (headless path), api.

use std::path::Path;

use crate::app::GameWorld;
use crate::render::{readback, Camera, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::Scene;

/// Default screenshot dimensions (16:9), matching the editor window aspect.
pub const DEFAULT_WIDTH: u32 = 1280;
pub const DEFAULT_HEIGHT: u32 = 720;

/// Render `scene` from `camera` into a PNG at `path`, `width` x `height`.
///
/// Returns `Ok(true)` when a frame was captured and written, `Ok(false)` when no
/// GPU/software adapter is available (skipped gracefully), or `Err` only on an
/// actual I/O / encode failure once a frame has been produced.
pub fn capture(
    scene: &Scene,
    camera: &Camera,
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
) -> Result<bool, String> {
    let mut renderer = match pollster::block_on(Renderer::new_headless(width, height)) {
        Some(r) => r,
        None => {
            log::warn!(
                "[Screenshot] no GPU/software adapter available — skipping capture of {}",
                path.as_ref().display()
            );
            return Ok(false);
        }
    };

    let (width, height) = (renderer.config.width, renderer.config.height);

    // Offscreen colour target the scene pass draws into, then we copy back from.
    let target = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Screenshot Target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OFFSCREEN_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

    // A per-view target bundle (own depth + post-FX) sized to the screenshot (#355).
    let mut view = RenderView::targetless(
        &renderer.device,
        OFFSCREEN_FORMAT,
        width,
        height,
        renderer.quality.bloom_divisor(),
    );

    // Reuse the editor's exact render path (editor_mode = false: no gizmos/grid).
    renderer.render(&mut view, scene, camera, &target_view, false, &[]);

    let pixels =
        readback::read_texture_rgba8(&renderer.device, &renderer.queue, &target, width, height);
    readback::write_png(path, width, height, &pixels)?;
    Ok(true)
}

/// Convenience: screenshot the harness's live `GameWorld` (its scene + camera).
pub fn capture_world(
    world: &GameWorld,
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
) -> Result<bool, String> {
    let scene = world.scene().borrow();
    capture(&scene, &world.camera().borrow(), path, width, height)
}
