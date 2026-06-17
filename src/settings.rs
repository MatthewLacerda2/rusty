//! src/settings.rs — platform-layer glue for runtime video + quality settings.
//!
//! Binary-local module (issue #89). The settings *surface* (the `Video` and
//! `Graphics` script namespaces) and the *persistence* (`Storage`) live in the
//! library; this module is the platform layer that ties them to the real wgpu
//! surface + winit window. It owns three boundaries:
//!
//! * **Load + apply at startup** ([`load`]) — read the persisted `video` blob and
//!   the `graphics.quality` tier from `Storage` (a boundary read, like the scene),
//!   seed the shared cells, and apply them to the renderer/window so the app boots
//!   at the saved resolution / vsync / fullscreen / tier.
//! * **Apply per frame** ([`apply_pending`]) — read the cells a script (or the
//!   editor) may have written this frame and reconfigure the surface (resolution /
//!   present mode) + window (fullscreen) only for what actually changed.
//! * **Persist at a boundary** ([`persist`]) — write the live settings back into
//!   `Storage`; the existing `flush_storage` on Stop / quit pushes them to disk.
//!
//! Everything here is a ONE-WAY write into render-only state, never read by
//! `FixedUpdate`, so the deterministic sim is unaffected (`main.rs`/`render` are
//! the determinism-exempt platform layer).

use std::sync::Arc;

use rusty::app::GameWorld;
use rusty::core::storage::Storage;
use rusty::core::video::{VideoSettings, VIDEO_NAMESPACE};
use rusty::render::postfx::QualityPreset;

use crate::Frontend;

/// `Storage` namespace + key the quality tier persists under (shares the
/// `Graphics` post-FX category, key `quality`).
const QUALITY_NAMESPACE: &str = "graphics";
const QUALITY_KEY: &str = "quality";

/// Read persisted settings from `Storage` and apply them to the renderer, window,
/// and the shared script cells, so the app boots at the saved resolution / vsync /
/// fullscreen / quality tier. Missing keys keep the boot defaults.
pub fn load(frontend: &mut Frontend, window: &Arc<winit::window::Window>, game: &GameWorld) {
    let (video, quality) = read(&game.resources.storage.borrow());

    // Seed the shared cells so the next script read sees the persisted values.
    *game.script_manager().video_cell().borrow_mut() = video;
    *game.script_manager().quality_cell().borrow_mut() = quality;
    frontend.editor_ui.quality_preset = quality;

    apply_video(frontend, window, video, &VideoSettings::default());
    frontend.renderer.set_quality(quality);
    frontend.applied_video = video;
}

/// Read `(video, quality)` from the store, each falling back to its default.
fn read(storage: &Storage) -> (VideoSettings, QualityPreset) {
    let video = storage
        .get_namespace(VIDEO_NAMESPACE)
        .map(|blob| VideoSettings::from_json(&blob))
        .unwrap_or_default();
    let quality = storage
        .get(QUALITY_NAMESPACE, QUALITY_KEY)
        .and_then(|v| v.as_str().and_then(parse_quality))
        .unwrap_or_default();
    (video, quality)
}

/// Apply any video-settings change a script wrote into the shared cell this frame.
/// Reconfigures only what changed (resolution / present mode / fullscreen). The
/// actually-effective vsync is written back into the cell, so a request the surface
/// can't honor (no `Immediate`) reflects the true state on the next `GetVsync`.
pub fn apply_pending(
    frontend: &mut Frontend,
    window: &Arc<winit::window::Window>,
    game: &GameWorld,
) {
    let requested = *game.script_manager().video_cell().borrow();
    let previous = frontend.applied_video;
    if !requested.diff(&previous).any() {
        return;
    }
    apply_video(frontend, window, requested, &previous);

    // Reflect the vsync the surface actually adopted back into the shared cell.
    let effective = VideoSettings {
        vsync: frontend.renderer.vsync(),
        ..requested
    };
    *game.script_manager().video_cell().borrow_mut() = effective;
    frontend.applied_video = effective;
}

/// Reconfigure the renderer + window for `next`, doing only the work `next.diff`
/// against `previous` flags. Shared by [`load`] (vs defaults) and [`apply_pending`].
fn apply_video(
    frontend: &mut Frontend,
    window: &Arc<winit::window::Window>,
    next: VideoSettings,
    previous: &VideoSettings,
) {
    let change = next.diff(previous);
    if change.fullscreen {
        let mode = next
            .fullscreen
            .then_some(winit::window::Fullscreen::Borderless(None));
        window.set_fullscreen(mode);
    }
    if change.vsync {
        frontend.renderer.set_vsync(next.vsync);
    }
    if change.resolution {
        let (w, h) = next.resolution();
        // Windowed: ask the OS for the new inner size; the resulting `Resized`
        // event reconfigures the surface. Fullscreen ignores inner-size requests,
        // so reconfigure the surface directly to the requested framebuffer.
        if next.fullscreen {
            frontend
                .renderer
                .resize(winit::dpi::PhysicalSize::new(w, h));
        } else {
            let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(w, h));
        }
    }
}

/// Write the live settings back into `Storage`. The existing `flush_storage` (Stop
/// / quit boundary) persists them to disk; this only updates the in-memory map.
pub fn persist(frontend: &Frontend, game: &GameWorld) {
    let video = *game.script_manager().video_cell().borrow();
    let quality = frontend.renderer.quality;
    let mut storage = game.resources.storage.borrow_mut();
    storage.set_namespace(VIDEO_NAMESPACE, video.to_json());
    storage.set(
        QUALITY_NAMESPACE,
        QUALITY_KEY,
        serde_json::Value::String(quality_name(quality).to_string()),
    );
}

/// `"Low"` / `"Medium"` / `"High"` -> tier (case-insensitive); unknown -> `None`.
fn parse_quality(name: &str) -> Option<QualityPreset> {
    match name.to_ascii_lowercase().as_str() {
        "low" => Some(QualityPreset::Low),
        "medium" => Some(QualityPreset::Medium),
        "high" => Some(QualityPreset::High),
        _ => None,
    }
}

/// The canonical string name of a quality tier, for persistence.
fn quality_name(p: QualityPreset) -> &'static str {
    match p {
        QualityPreset::Low => "Low",
        QualityPreset::Medium => "Medium",
        QualityPreset::High => "High",
    }
}
