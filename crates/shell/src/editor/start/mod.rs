//! crates/shell/src/editor/start/ — how the editor starts: on a named project, or on the
//! project picker first.

mod picker;

#[cfg(feature = "dev")]
pub use picker::capture_picker;

use super::{icon, EditorFrontend};
use crate::{boot, Launch, Shell};
use rusty_app::GameWorld;
use rusty_app::ProjectSource;
use rusty_core::project::bench::recent;
use rusty_core::project::{Access, BootPlan, Opened};
use rusty_core::settings::video::VideoSettings;
use rusty_editor::project_picker::ProjectPicker;

/// The editor window's title once a project is open.
const TITLE: &str = "Rusty 3D Game Engine & Editor";
/// The window's title while the picker shows.
const PICKER_TITLE: &str = "rusty — Projects";
/// The editor window's desktop identity on Linux: the name of the `rusty.desktop`
const APP_ID: &str = "rusty";

/// Boot the editor on the project `opened` and run until the window closes.
pub fn launch(opened: &Opened) {
    let (game, launch) = prepare(opened);
    crate::run(game, launch);
}

/// Show the project picker, then boot the editor on the project it opens.
pub fn launch_picker() {
    let size = VideoSettings::default().resolution();
    crate::run_after(
        PICKER_TITLE,
        Some(APP_ID),
        size,
        true,
        |window, renderer| {
            icon::apply(window);
            picker::PickerStage::new(window, renderer, ProjectPicker::for_user())
        },
        |opened: Opened| prepare(&opened),
    );
}

fn prepare(
    opened: &Opened,
) -> (
    GameWorld,
    Launch<impl FnOnce(&Shell, &GameWorld) -> EditorFrontend>,
) {
    println!("[Engine] Starting rusty 3D engine...");
    if let Err(err) = recent::record(&opened.root, unix_now()) {
        eprintln!("[Project] could not update the recent projects: {err}");
    }
    rusty_edit::print_refresh(&rusty_edit::seed_project());
    let startup = &opened.settings.build.startup_scene;
    let plan = rusty_core::project::boot_scene(&opened.root, startup, Access::Edit);
    let game = world(&plan.unwrap_or_default(), ProjectSource::bound(opened));
    if let Some(warning) = opened.engine.warning() {
        game.console()
            .borrow_mut()
            .warn(format!("[Project] {warning}"));
    }

    let launch = Launch {
        title: TITLE.to_string(),
        app_id: Some(APP_ID),
        maximized: true,
        video_defaults: VideoSettings::default(),
        frontend: |shell: &Shell, game: &GameWorld| {
            icon::apply(&shell.window);
            EditorFrontend::new(shell, game)
        },
    };
    (game, launch)
}

fn world(plan: &BootPlan, project: ProjectSource) -> GameWorld {
    let mut game = boot::load_game(plan);
    let init = game.init_edit_runtime();
    game.report_lua_init(init);
    game.open_project(project);
    game
}

/// Now, in Unix seconds (the shell is platform layer, so it may read the clock): when a project was opened, and what the picker dates by.
fn unix_now() -> u64 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    now.map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_app::project_fixture::{assert_runs_the_fixture, FIXTURE};

    #[test]
    fn the_editor_world_binds_the_projects_settings() {
        let path = rusty_test_support::temp::dir().join("project.rusty");
        std::fs::write(&path, FIXTURE).unwrap();
        let game = world(&BootPlan::default(), ProjectSource::Bound(path.clone()));
        assert_runs_the_fixture(&game);
        let mut app = game.resources.application.borrow_mut();
        app.update(|b| b.product_name = "Arena".into()).unwrap();
        let file = rusty_core::project::ProjectFile::read(&path)
            .unwrap()
            .unwrap();
        assert_eq!(
            file.build.product_name, "Arena",
            "the editor writes the file"
        );
    }
}
