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
use crate::core::project::{Access, ProjectSettings};
use crate::core::settings::video::VideoSettings;
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
        // `Graphics.SetQuality` / `SetOption` reach the renderer here — there is no
        // editor dropdown to reconcile with.
        super::settings::apply_graphics(shell, game);

        let device = &shell.renderer.device;
        let (width, height) = (shell.renderer.config.width, shell.renderer.config.height);
        // The UI lays out on the game view's pixel size (#417): here, the whole window.
        game.resources
            .screen
            .borrow_mut()
            .set_game_view(width, height);
        let view = match &mut self.view {
            Some(view) => view,
            slot @ None => slot.insert(RenderView::targetless(
                device,
                shell.renderer.config.format,
                width,
                height,
            )),
        };
        // The screen renders its scene at the player's render scale (#1498).
        view.set_render_scale(shell.renderer.graphics(false).render_scale);
        view.resize(device, width, height);

        // The view is targetless (it renders straight onto the swapchain), so hand it
        // the frame to draw the in-game UI on (#418).
        view.set_ui_output(Some(shell.renderer.surface_ui_view(frame)));

        let scene = game.scene().borrow();
        let camera = crate::scene::game_camera_from_scene(&game.camera().borrow(), &scene);
        // No debug overlays in a shipped game: editor mode off.
        let textures = std::rc::Rc::clone(game.resources.screen.borrow().textures());
        shell.renderer.set_ui_textures(textures);
        shell.renderer.render(view, &scene, &camera, target, false);
        drop(scene);
        // `Debug.GetAdaptedExposure` reads the screen's adapted exposure (#1444).
        #[cfg(feature = "dev")]
        {
            game.scene().borrow_mut().screen_exposure = view.exposure_readout();
        }
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

/// Boot the standalone player on the open project's settings
/// (`core::project::Opened`) and run until the game quits or the window closes.
pub fn launch(settings: ProjectSettings) {
    let build = settings.build.clone();
    boot::seed_project_workspace();
    println!(
        "[Player] Starting {} ({})...",
        build.product_name, build.startup_scene
    );

    let root = crate::core::project::root();
    let scene = crate::core::project::boot_scene(&root, &build.startup_scene, Access::Run);
    let mut game = boot::load_game(&scene);
    // Unbound: a shipped game reads its settings but never rewrites them. The
    // graphics defaults are what a first launch's options fall back to (#1438).
    game.resources.application.borrow_mut().run_with(settings);
    game.boot_standalone();

    let launch = Launch {
        video_defaults: video_defaults(&build),
        title: build.product_name,
        // The game's own `.desktop` entry is its packager's to name.
        app_id: None,
        maximized: false,
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
