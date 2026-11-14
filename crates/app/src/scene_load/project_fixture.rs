//! The fixture project every entry point's test opens (#1652): this crate's
//! `project_tests` and the root's editor and player starts. A test hook, compiled in
//! every build so the root's tests reach it (crates/CLAUDE.md, *Test hooks*).

use crate::GameWorld;

/// A project file whose startup scene and initial quality preset are not the
/// engine's defaults.
pub const FIXTURE: &str = r#"{
    "build": { "startup_scene": "assets/scenes/arena.scene" },
    "quality": { "default": "High" }
}"#;

/// The world answers the fixture's settings, through `Application` and the
/// graphics cell the renderer reads.
pub fn assert_runs_the_fixture(game: &GameWorld) {
    let app = game.resources.application.borrow();
    assert_eq!(app.build().startup_scene, "assets/scenes/arena.scene");
    assert_eq!(app.quality(), Some("High"));
    let high = app
        .quality_presets()
        .get("High")
        .expect("the fixture names a known preset")
        .values;
    let graphics = game.script_manager().graphics_cell();
    assert_eq!(graphics.borrow().quality, high, "the preset runs at once");
}
