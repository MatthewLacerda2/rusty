//! src/shell/editor/start/ — how the editor starts: on a named project, or on the
//! project picker first (#854).
//!
//! `rusty --project <dir>` opens that project straight away ([`launch`]), as Unity
//! skips the Hub on `-projectPath`. Plain `rusty` shows the picker on the window
//! ([`launch_picker`]); the project it opens boots the editor on the same window.
//! Either way the opened project goes to the top of the user's recent list.

mod picker;

#[cfg(feature = "dev")]
pub use picker::capture_picker;

use super::{icon, EditorFrontend};
use crate::app::GameWorld;
use crate::core::project::bench::recent;
use crate::core::project::{Access, Opened, PROJECT_FILE};
use crate::core::settings::video::VideoSettings;
use crate::editor::project_picker::ProjectPicker;
use crate::shell::{boot, Launch, Shell};

/// The editor window's title once a project is open.
const TITLE: &str = "Rusty 3D Game Engine & Editor";
/// The window's title while the picker shows.
const PICKER_TITLE: &str = "rusty — Projects";
/// The editor window's desktop identity on Linux: the name of the `rusty.desktop`
/// entry `make launcher` installs (`tools/launcher/launcher.py`'s `APP_ID`, a test
/// there holds the two equal), so the dock shows the logo, which Wayland takes from
/// that entry only, and groups the window with the launcher (#1315).
const APP_ID: &str = "rusty";

/// Boot the editor on the project `opened` and run until the window closes.
pub fn launch(opened: &Opened) {
    let (game, launch) = prepare(opened);
    crate::shell::run(game, launch);
}

/// Show the project picker, then boot the editor on the project it opens.
pub fn launch_picker() {
    let size = VideoSettings::default().resolution();
    crate::shell::run_after(
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

/// Boot the editor's game on the open project's boot scene
/// (`core::project::boot_scene`: the scene this machine last had open, then the
/// startup scene, then an empty one, #1618). An engine mismatch is logged to the
/// editor console as well as stderr.
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
    boot::seed_project_workspace();
    let startup = &opened.settings.build.startup_scene;
    let scene = crate::core::project::boot_scene(&opened.root, startup, Access::Edit);
    let mut game = boot::load_game(&scene);
    // Editor scripting works outside Play, as in Unity (#1339): the console line
    // and MCP `eval` drive the edit scene; Stop brings this runtime back.
    let init = game.init_edit_runtime();
    game.report_lua_init(init);
    // Bound: the File → Build Settings window and `Application.Set*` write the file.
    let bound = game.resources.application.borrow_mut().open(PROJECT_FILE);
    if let Err(err) = bound {
        game.console().borrow_mut().error(err);
    }
    game.apply_project_settings(); // edit mode runs on them too (#1517)
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

/// Now, in Unix seconds (the shell is platform layer, so it may read the clock): when a project was opened, and what the picker dates by.
fn unix_now() -> u64 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    now.map_or(0, |d| d.as_secs())
}
