use super::Shell;
use rusty_app::GameWorld;
use rusty_audio::SpeakerMode;
use rusty_core::settings::video::{VideoSettings, VIDEO_NAMESPACE};
use rusty_core::settings::{saved_quality, GraphicsSettings};

pub fn load(shell: &mut Shell, game: &GameWorld, defaults: VideoSettings) {
    let storage = game.resources.storage.borrow();
    let video = storage
        .get_namespace(VIDEO_NAMESPACE)
        .map(|blob| VideoSettings::from_json(&blob))
        .unwrap_or(defaults);
    let mut app = game.resources.application.borrow_mut();
    app.restore_quality(saved_quality(&storage));
    let defaults = (app.graphics_defaults(), app.render_scale_default());
    let graphics = GraphicsSettings::load(&storage, defaults, app.quality_presets());
    drop(app);
    let speaker_mode = SpeakerMode::load(&storage);
    drop(storage);
    game.resources
        .audio
        .borrow_mut()
        .set_speaker_mode(speaker_mode);

    *game.script_manager().video_cell().borrow_mut() = video;
    *game.script_manager().graphics_cell().borrow_mut() = graphics;

    report_display_scale(shell, game);
    apply_video(shell, video, &VideoSettings::default());
    apply_graphics(shell, game);
    shell.applied_video = video;
}

/// Hand the shared graphics cell (preset values + player options + render scale) to the
/// renderer: the one place either frontend does it, every frame, and [`load`] at boot
///. The render scale resolves on the display scale the screen resource holds
///, which each frontend reports first ([`report_display_scale`]).
pub fn apply_graphics(shell: &mut Shell, game: &GameWorld) {
    let display_scale = game.resources.screen.borrow().display_scale();
    let graphics = *game.script_manager().graphics_cell().borrow();
    shell.renderer.apply_graphics(graphics, display_scale);
}

pub fn report_display_scale(shell: &Shell, game: &GameWorld) {
    let scale = shell.window.scale_factor() as f32;
    game.resources.screen.borrow_mut().set_display_scale(scale);
}

/// Apply any video-settings change a script wrote into the shared cell this frame.
pub fn apply_pending(shell: &mut Shell, game: &GameWorld) {
    let requested = *game.script_manager().video_cell().borrow();
    let previous = shell.applied_video;
    if !requested.diff(&previous).any() {
        return;
    }
    apply_video(shell, requested, &previous);

    let effective = VideoSettings {
        vsync: shell.renderer.vsync(),
        ..requested
    };
    *game.script_manager().video_cell().borrow_mut() = effective;
    shell.applied_video = effective;
}

/// Reconfigure the renderer + window for `next`, doing only the work `next.diff`
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
        if next.fullscreen {
            shell.renderer.resize(rusty_render::PixelSize::new(w, h));
        } else {
            let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(w, h));
        }
    }
}

/// Write the live settings back into `Storage` at quit. The graphics settings are
/// also written at Stop (`Resources::store_graphics`); the existing `flush_storage`
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
