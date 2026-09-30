//! src/shell/settings.rs — platform-layer glue for runtime video + quality settings.
//!
//! Issue #89; shared by both frontends since #431. The settings *surface* (the
//! `Video` and `Graphics` script namespaces) and the *persistence* (`Storage`) live
//! in the sim; this module is the platform layer that ties them to the real wgpu
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
//! `FixedUpdate`, so the deterministic sim is unaffected (the shell and `render` are
//! the determinism-exempt platform layer).

use super::Shell;
use crate::app::GameWorld;
use crate::core::quality::QualityPreset;
use crate::core::storage::Storage;
use crate::core::video::{VideoSettings, VIDEO_NAMESPACE};

/// `Storage` namespace + key the quality tier persists under (shares the
/// `Graphics` post-FX category, key `quality`).
const QUALITY_NAMESPACE: &str = "graphics";
const QUALITY_KEY: &str = "quality";

/// Read persisted settings from `Storage` and apply them to the renderer, window,
/// and the shared script cells, so the app boots at the saved resolution / vsync /
/// fullscreen / quality tier. Missing video settings fall back to `defaults` (the
/// player's first launch honours the build's window mode); a missing tier keeps the
/// default tier.
pub fn load(shell: &mut Shell, game: &GameWorld, defaults: VideoSettings) {
    let (video, quality) = read(&game.resources.storage.borrow(), defaults);

    // Seed the shared cells so the next script read sees the persisted values.
    *game.script_manager().video_cell().borrow_mut() = video;
    *game.script_manager().quality_cell().borrow_mut() = quality;

    apply_video(shell, video, &VideoSettings::default());
    shell.renderer.set_quality(quality);
    shell.applied_video = video;
}

/// Read `(video, quality)` from the store; video falls back to `defaults`, the tier
/// to its default.
fn read(storage: &Storage, defaults: VideoSettings) -> (VideoSettings, QualityPreset) {
    let video = storage
        .get_namespace(VIDEO_NAMESPACE)
        .map(|blob| VideoSettings::from_json(&blob))
        .unwrap_or(defaults);
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
pub fn apply_pending(shell: &mut Shell, game: &GameWorld) {
    let requested = *game.script_manager().video_cell().borrow();
    let previous = shell.applied_video;
    if !requested.diff(&previous).any() {
        return;
    }
    apply_video(shell, requested, &previous);

    // Reflect the vsync the surface actually adopted back into the shared cell.
    let effective = VideoSettings {
        vsync: shell.renderer.vsync(),
        ..requested
    };
    *game.script_manager().video_cell().borrow_mut() = effective;
    shell.applied_video = effective;
}

/// Reconfigure the renderer + window for `next`, doing only the work `next.diff`
/// against `previous` flags. Shared by [`load`] (vs defaults) and [`apply_pending`].
fn apply_video(shell: &mut Shell, next: VideoSettings, previous: &VideoSettings) {
    let window = &shell.window;
    let change = next.diff(previous);
    if change.fullscreen {
        let mode = next
            .fullscreen
            .then_some(winit::window::Fullscreen::Borderless(None));
        window.set_fullscreen(mode);
    }
    if change.vsync {
        shell.renderer.set_vsync(next.vsync);
    }
    if change.resolution {
        let (w, h) = next.resolution();
        // Windowed: ask the OS for the new inner size; the resulting `Resized`
        // event reconfigures the surface. Fullscreen ignores inner-size requests,
        // so reconfigure the surface directly to the requested framebuffer.
        if next.fullscreen {
            shell.renderer.resize(winit::dpi::PhysicalSize::new(w, h));
        } else {
            let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(w, h));
        }
    }
}

/// Write the live settings back into `Storage`. The existing `flush_storage` (Stop
/// / quit boundary) persists them to disk; this only updates the in-memory map.
pub fn persist(shell: &Shell, game: &GameWorld) {
    let video = *game.script_manager().video_cell().borrow();
    let quality = shell.renderer.quality;
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
