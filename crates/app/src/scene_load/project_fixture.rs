use crate::GameWorld;

pub const FIXTURE: &str = r#"{
    "build": { "startup_scene": "assets/scenes/arena.scene" },
    "quality": { "default": "High" }
}"#;

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
