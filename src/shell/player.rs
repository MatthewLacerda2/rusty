//! src/shell/player.rs — the standalone player frontend (#431): a game with no editor.
//!
//! Boot: read the project's build settings, load the startup scene, enter Play at once
//! (no edit snapshot — there is no edit mode to return to), apply the persisted video
//! and keybinding settings, run. Each frame renders the active camera stack and the
//! in-game UI straight onto the swapchain with no chrome, and the game gets all input. `Application.Quit()`
//! closes the window; the loop's exit flushes `Storage` first.

use super::frame::Host;
use super::{boot, Frontend, Launch, Shell};
use crate::app::GameWorld;
use crate::core::application::{BuildSettings, WindowMode};
use crate::core::project::{self, ProjectFile};
use crate::core::video::VideoSettings;
use crate::render::RenderView;

/// The player's frontend state: the one full-window render view.
#[derive(Default)]
pub struct PlayerFrontend {
    /// Sized to the swapchain every frame; created on the first frame.
    view: Option<RenderView>,
}

impl Frontend for PlayerFrontend {
    const HOST: Host = Host::Player;

    fn draw(
        &mut self,
        shell: &mut Shell,
        game: &mut GameWorld,
        frame: &wgpu::Texture,
        target: &wgpu::TextureView,
    ) {
        // `Graphics.SetQuality` reaches the renderer here — there is no editor dropdown
        // to reconcile with (`set_quality` guards the bloom realloc).
        let quality = *game.script_manager().quality_cell().borrow();
        shell.renderer.set_quality(quality);

        let device = &shell.renderer.device;
        let (width, height) = (shell.renderer.config.width, shell.renderer.config.height);
        let bloom = shell.renderer.quality.bloom_divisor();
        // The UI lays out on the game view's pixel size (#417): here, the whole window.
        game.resources
            .screen
            .borrow_mut()
            .set_game_view(width, height);
        let view = match &mut self.view {
            Some(view) => {
                view.resize(device, width, height, bloom);
                view
            }
            slot @ None => slot.insert(RenderView::targetless(
                device,
                shell.renderer.config.format,
                width,
                height,
                bloom,
            )),
        };

        // The view is targetless (it renders straight onto the swapchain), so hand it
        // the frame to draw the in-game UI on (#418).
        view.set_ui_output(Some(shell.renderer.surface_ui_view(frame)));

        let scene = game.scene().borrow();
        let camera = crate::scene::game_camera_from_scene(&game.camera().borrow(), &scene);
        // No debug overlays in a shipped game: editor mode off.
        shell.renderer.render(view, &scene, &camera, target, false);
    }
}

/// What a first launch opens with: the default resolution in the build's window mode.
/// Saved `Video` settings, once a player changes them, win over this.
pub fn video_defaults(build: &BuildSettings) -> VideoSettings {
    VideoSettings {
        fullscreen: build.window_mode == WindowMode::Fullscreen,
        ..VideoSettings::default()
    }
}

/// Boot the standalone player and run until the game quits or the window closes.
pub fn launch() {
    boot::seed_project_workspace();
    let build = match ProjectFile::load(&project::root()) {
        Ok(file) => file.build,
        Err(err) => {
            eprintln!("[Player] {err} — using the default build settings");
            BuildSettings::default()
        }
    };
    println!(
        "[Player] Starting {} ({})...",
        build.product_name, build.startup_scene
    );

    let mut game = boot::load_game(&build.startup_scene);
    // Unbound: a shipped game reads its build settings but never rewrites them.
    let _ = game
        .resources
        .application
        .borrow_mut()
        .set_build(build.clone());
    game.boot_standalone();

    let launch = Launch {
        video_defaults: video_defaults(&build),
        title: build.product_name,
        frontend: |_: &Shell, _: &GameWorld| PlayerFrontend::default(),
    };
    super::run(game, launch);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_launch_follows_the_build_window_mode() {
        let mut build = BuildSettings::default();
        assert!(!video_defaults(&build).fullscreen);
        build.window_mode = WindowMode::Fullscreen;
        let video = video_defaults(&build);
        assert!(video.fullscreen);
        assert_eq!(video.resolution(), VideoSettings::default().resolution());
    }
}
