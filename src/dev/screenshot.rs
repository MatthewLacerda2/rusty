//! src/dev/screenshot.rs — Offscreen render -> PNG (the agent's eyes)
//!
//! Renders ONE frame through the same `Renderer::render` the editor uses, into a
//! [`RenderView`](crate::render::RenderView)'s own offscreen colour target, then hands
//! that target to `render::readback` for the copy-back + PNG encode. Lets an agent
//! literally SEE a frame and judge lighting / SSR / shadows against the CS1.6 ->
//! FEAR -> Trepang2 visual bar.
//!
//! **Two entry points, one path.** [`capture_into`] draws through a caller-owned
//! [`CaptureHost`] — the form to use when taking more than one shot, because the host's
//! renderer and view are built once and reused (#355 step 5). [`capture`] is the
//! one-shot convenience: it spins up a host, takes the shot, and drops it. A scenario
//! or test taking several shots should hold a host rather than call `capture` N times,
//! which is N wgpu devices.
//!
//! Runtime caveat: needs a GPU or software adapter (e.g. lavapipe) in the
//! container. When none is available, these return `Ok(false)` after a clear log
//! (`Err` under `RUSTY_REQUIRE_GPU=1`, #885) — they NEVER panic, and the rest of the dev layer (Step / StepUntil / state) needs no
//! GPU at all.

use std::path::Path;

use super::capture::CaptureHost;
use crate::app::GameWorld;
use crate::scene::Camera;
use crate::scene::Scene;

/// Default screenshot dimensions: the default windowed framebuffer (16:9).
pub use crate::core::video::{DEFAULT_HEIGHT, DEFAULT_WIDTH};

/// Render `scene` from `camera` into a PNG at `path`, `width` x `height`, through
/// `host`'s shared renderer.
///
/// Returns `Ok(true)` when a frame was captured and written, `Ok(false)` when no
/// GPU/software adapter is available (skipped gracefully), or `Err` on an I/O /
/// encode failure — or on a missing adapter under `RUSTY_REQUIRE_GPU=1` (#885).
pub fn capture_into(
    host: &mut CaptureHost,
    scene: &Scene,
    camera: &Camera,
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
) -> Result<bool, String> {
    // `draw` already warned, or failed under `RUSTY_REQUIRE_GPU=1` (#885).
    if host.draw(scene, camera, width, height)?.is_none() {
        return Ok(false);
    }
    host.write_png(path)?;
    Ok(true)
}

/// One-shot [`capture_into`]: builds a throwaway [`CaptureHost`], takes the shot, drops
/// it. Prefer holding a host when taking more than one.
pub fn capture(
    scene: &Scene,
    camera: &Camera,
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
) -> Result<bool, String> {
    capture_into(&mut CaptureHost::new(), scene, camera, path, width, height)
}

/// Convenience: screenshot the harness's live `GameWorld` (its scene + camera) through
/// `host`, so a scenario's shots all share one renderer.
pub fn capture_world_into(
    host: &mut CaptureHost,
    world: &GameWorld,
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
) -> Result<bool, String> {
    share_ui_textures(host, world, width, height);
    let scene = world.scene().borrow();
    capture_into(host, &scene, &world.camera().borrow(), path, width, height)
}

/// Hand `world`'s texture-size table to `host`'s renderer (#1221), so a capture
/// draws the UI exactly as the sim laid it out.
pub fn share_ui_textures(host: &mut CaptureHost, world: &GameWorld, width: u32, height: u32) {
    if let Some(renderer) = host.renderer(width, height) {
        let textures = world.resources.screen.borrow().textures().clone();
        renderer.set_ui_textures(textures);
    }
}
