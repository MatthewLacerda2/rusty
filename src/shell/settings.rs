//! src/shell/settings.rs — platform-layer glue for runtime video, quality and audio
//! settings.
//!
//! Issue #89; shared by both frontends since #431. The settings *surface* (the
//! `Video` and `Graphics` script namespaces) and the *persistence* (`Storage`) live
//! in the sim; this module is the platform layer that ties them to the real wgpu
//! surface + winit window. It owns three boundaries:
//!
//! * **Load + apply at startup** ([`load`]) — read the persisted `video` blob and
//!   the `graphics.quality` preset name (among the project's presets, #1522) and the
//!   `graphics.options` player graphics options (over the project's defaults, #1438) from `Storage` (a boundary read, like the
//!   scene), seed the shared cells, and apply them to the renderer/window so the app boots
//!   at the saved resolution / vsync / fullscreen / tier, and the audio maestro in
//!   the saved speaker mode (#546).
//! * **Apply per frame** ([`apply_pending`]) — read the cells a script (or the
//!   editor) may have written this frame and reconfigure the surface (resolution /
//!   present mode) + window (fullscreen) only for what actually changed.
//! * **Persist at a boundary** ([`persist`]) — write the live settings back into
//!   `Storage` at quit (the graphics settings at Stop too); the existing
//!   `flush_storage` on Stop / quit pushes them to disk.
//!
//! Everything here is a ONE-WAY write into render-only state, never read by
//! `FixedUpdate`, so the deterministic sim is unaffected (the shell and `render` are
//! the determinism-exempt platform layer).

use super::Shell;
use crate::app::GameWorld;
use crate::audio::SpeakerMode;
use crate::core::settings::video::{VideoSettings, VIDEO_NAMESPACE};
use crate::core::settings::{saved_quality, GraphicsSettings};

/// Read persisted settings from `Storage` and apply them to the renderer, window,
/// and the shared script cells, so the app boots at the saved resolution / vsync /
/// fullscreen / quality preset / player graphics options. Missing video settings fall
/// back to `defaults` (the player's first launch honours the build's window mode); a
/// missing (or removed) preset runs the project's initial one, a missing option the
/// project's default.
pub fn load(shell: &mut Shell, game: &GameWorld, defaults: VideoSettings) {
    let storage = game.resources.storage.borrow();
    let video = storage
        .get_namespace(VIDEO_NAMESPACE)
        .map(|blob| VideoSettings::from_json(&blob))
        .unwrap_or(defaults);
    let mut app = game.resources.application.borrow_mut();
    app.restore_quality(saved_quality(&storage));
    let graphics = GraphicsSettings::load(&storage, app.graphics_defaults(), app.quality_presets());
    drop(app);
    let speaker_mode = SpeakerMode::load(&storage);
    drop(storage);
    game.resources
        .audio
        .borrow_mut()
        .set_speaker_mode(speaker_mode);

    // Seed the shared cells so the next script read sees the persisted values.
    *game.script_manager().video_cell().borrow_mut() = video;
    *game.script_manager().graphics_cell().borrow_mut() = graphics;

    apply_video(shell, video, &VideoSettings::default());
    apply_graphics(shell, game);
    shell.applied_video = video;
}

/// Hand the shared graphics cell (preset values + player options) to the renderer: the one
/// place either frontend does it, every frame, and [`load`] at boot (#1438).
pub fn apply_graphics(shell: &mut Shell, game: &GameWorld) {
    let graphics = *game.script_manager().graphics_cell().borrow();
    shell.renderer.apply_graphics(graphics);
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
            shell.renderer.resize(crate::render::PixelSize::new(w, h));
        } else {
            let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(w, h));
        }
    }
}

/// Write the live settings back into `Storage` at quit. The graphics settings are
/// also written at Stop (`Resources::store_graphics`); the existing `flush_storage`
/// persists them all to disk; this only updates the in-memory map.
pub fn persist(game: &GameWorld) {
    game.resources.store_graphics();
    let video = *game.script_manager().video_cell().borrow();
    let mut storage = game.resources.storage.borrow_mut();
    game.resources
        .audio
        .borrow()
        .speaker_mode()
        .store(&mut storage);
    storage.set_namespace(VIDEO_NAMESPACE, video.to_json());
}
