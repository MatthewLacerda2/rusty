//! crates/shell/src/player/ — the standalone player frontend: a game with no editor.

use super::frame::Host;
use super::{boot, Frontend, Launch, Shell};
use rusty_app::{GameWorld, ProjectSource};
use rusty_core::application::{BuildSettings, WindowMode};
use rusty_core::project::{Access, ProjectSettings};
use rusty_core::settings::video::VideoSettings;
use rusty_render::RenderView;
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
        super::settings::report_display_scale(shell, game);
        super::settings::apply_graphics(shell, game);

        let device = &shell.renderer.device;
        let (width, height) = (shell.renderer.config.width, shell.renderer.config.height);
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
        view.set_render_scale(shell.renderer.render_scale(false));
        view.resize(device, width, height);

        view.set_ui_output(Some(shell.renderer.surface_ui_view(frame)));

        let camera = game.main_view();
        let scene = game.scene().borrow();
        let textures = std::rc::Rc::clone(game.resources.screen.borrow().textures());
        shell.renderer.set_ui_textures(textures);
        shell.renderer.render(view, &scene, &camera, target, false);
        drop(scene);
        #[cfg(feature = "dev")]
        {
            game.scene().borrow_mut().screen_exposure = view.exposure_readout();
        }
    }
}

/// What a first launch opens with: the default resolution in the build's window mode.
pub fn video_defaults(build: &BuildSettings) -> VideoSettings {
    VideoSettings {
        fullscreen: build.window_mode == WindowMode::Fullscreen,
        ..VideoSettings::default()
    }
}

fn world(plan: &rusty_core::project::BootPlan, settings: ProjectSettings) -> GameWorld {
    rusty_asset::convert::mark_standalone();
    let mut game = boot::load_game(plan);
    game.open_project(ProjectSource::ReadOnly(settings));
    game.boot_standalone();
    game
}

/// The player's world on the open project's startup scene, read-only: everything
/// [`launch`] does before it opens the window. Errs when no startup scene is set.
fn boot(settings: ProjectSettings) -> Result<GameWorld, String> {
    // A run reads no editor record, so the root is moot.
    let startup = &settings.build.startup_scene;
    let plan = rusty_core::project::boot_scene(Path::new(""), startup, Access::Run)?;
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
        use rusty_app::project_fixture::{assert_runs_the_fixture, FIXTURE};
        let file = rusty_core::project::ProjectFile::from_json(FIXTURE).unwrap();
        let game = world(&rusty_core::project::BootPlan::default(), file.into());
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
