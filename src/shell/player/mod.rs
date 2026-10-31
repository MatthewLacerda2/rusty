//! src/shell/player/ — the standalone player frontend (#431): a game with no editor.
//!
//! Boot: read the project's build settings, load the startup scene, enter Play at once
//! (no edit snapshot — there is no edit mode to return to), apply the persisted video
//! and keybinding settings, run. Each frame renders the active camera stack and the
//! in-game UI straight onto the swapchain with no chrome, and the game gets all input. `Application.Quit()`
//! closes the window; the loop's exit flushes `Storage` first.
//!
//! **The player boots read-only** (#1653), as Unity's player never writes into its
//! data folder: no seeding and no asset refresh, which the editor's open
//! (`boot::seed_project_workspace`) and the export (`dev::export`) do instead. What
//! it writes goes under `saved/` (`Storage`, a dev build's demos), Unity's
//! `persistentDataPath`; `read_only_tests` holds it on an export.

use super::frame::Host;
use super::{boot, Frontend, Launch, Shell};
use crate::app::{GameWorld, ProjectSource};
use crate::core::application::{BuildSettings, WindowMode};
use crate::core::project::{Access, ProjectSettings};
use crate::core::settings::video::VideoSettings;
use crate::render::RenderView;
use std::path::Path;

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

        let camera = game.main_view();
        let scene = game.scene().borrow();
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

/// The player's world on `plan`'s scene: the project's settings taken unbound (a
/// shipped game reads them but never rewrites them; the graphics defaults are what
/// a first launch's options fall back to, #1438), then Play entered for good.
fn world(plan: &crate::core::project::BootPlan, settings: ProjectSettings) -> GameWorld {
    let mut game = boot::load_game(plan);
    game.open_project(ProjectSource::ReadOnly(settings));
    game.boot_standalone();
    game
}

/// The player's world on the open project's startup scene, read-only: everything
/// [`launch`] does before it opens the window. Errs when no startup scene is set.
fn boot(settings: ProjectSettings) -> Result<GameWorld, String> {
    // A run reads no editor record, so the root is moot (`boot_scene`, #1618).
    let startup = &settings.build.startup_scene;
    let plan = crate::core::project::boot_scene(Path::new(""), startup, Access::Run)?;
    Ok(world(&plan, settings))
}

/// Boot the standalone player on the open project's settings
/// (`core::project::Opened`) and run until the game quits or the window closes.
pub fn launch(settings: ProjectSettings) {
    let build = settings.build.clone();
    println!(
        "[Player] Starting {} ({})...",
        build.product_name, build.startup_scene
    );

    let game = boot(settings).unwrap_or_else(|err| {
        eprintln!("[Player] cannot start: {err}");
        std::process::exit(1);
    });

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

#[cfg(all(test, feature = "dev"))]
mod read_only_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_player_world_runs_the_projects_settings() {
        use crate::app::project_tests::{assert_runs_the_fixture, FIXTURE};
        let file = crate::core::project::ProjectFile::from_json(FIXTURE).unwrap();
        let game = world(&crate::core::project::BootPlan::default(), file.into());
        assert_runs_the_fixture(&game);
    }

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
