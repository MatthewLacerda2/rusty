//! The render scale (#1498, #1715): `Graphics.Set/GetRenderScale` write the player's
//! choice in the live graphics cell the renderer reads, clamped to URP's range;
//! with none, the project's default runs (`Get/SetDefaultRenderScale`), resolved on
//! the display scale `Video.GetDisplayScale` reads.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::api::bind;
use rusty::core::application::Application;
use rusty::core::settings::render_scale::{RenderScale, RenderScaleDefault};
use rusty::core::settings::GraphicsSettings;
use rusty::scene::Scene;
use rusty::ui::ScreenSize;

type Cells = (
    Rc<RefCell<GraphicsSettings>>,
    Rc<RefCell<ScreenSize>>,
    Rc<RefCell<Application>>,
);

/// A VM with `Graphics` and `Video` over fresh cells.
fn vm() -> (Lua, Cells) {
    let (lua, scene) = (Lua::new(), Rc::new(RefCell::new(Scene::new())));
    let graphics = Rc::new(RefCell::new(GraphicsSettings::default()));
    let (screen, app, video) = (Rc::default(), Rc::default(), Rc::default());
    let screen_cell = bind(&lua, &screen);
    let (gfx, app_cell) = (bind(&lua, &graphics), bind(&lua, &app));
    rusty::api::graphics::register(&lua, bind(&lua, &scene), (gfx, screen_cell), app_cell).unwrap();
    rusty::api::video::register(&lua, bind(&lua, &video), screen_cell).unwrap();
    (lua, (graphics, screen, app))
}

fn get(lua: &Lua) -> f32 {
    lua.load("return Graphics.GetRenderScale()").eval().unwrap()
}

#[test]
fn set_render_scale_writes_the_cell_clamped_and_reads_back() {
    let (lua, (graphics, ..)) = vm();
    assert_eq!(get(&lua), 1.0, "native by default");
    lua.load("Graphics.SetRenderScale(0.5)").exec().unwrap();
    assert_eq!(
        graphics.borrow().render_scale.chosen,
        Some(RenderScale::new(0.5))
    );
    assert_eq!(get(&lua), 0.5);
    lua.load("Graphics.SetRenderScale(9)").exec().unwrap();
    assert_eq!(get(&lua), RenderScale::MAX);
    lua.load("Graphics.SetRenderScale(0)").exec().unwrap();
    assert_eq!(get(&lua), RenderScale::MIN);
}

#[test]
fn the_default_runs_until_the_player_chooses_and_after_a_reset() {
    let (lua, (graphics, _, app)) = vm();
    lua.load("Graphics.SetDefaultRenderScale(0.7)")
        .exec()
        .unwrap();
    let fixed = RenderScaleDefault::Fixed(RenderScale::new(0.7));
    assert_eq!(app.borrow().render_scale_default(), fixed);
    assert_eq!(get(&lua), 0.7, "no choice: the default runs at once");
    let read: f32 = lua
        .load("return Graphics.GetDefaultRenderScale()")
        .eval()
        .unwrap();
    assert_eq!(read, 0.7);
    lua.load("Graphics.SetRenderScale(0.5)").exec().unwrap();
    assert_eq!(get(&lua), 0.5, "the player's choice wins");
    lua.load("Graphics.ResetRenderScale()").exec().unwrap();
    assert_eq!(graphics.borrow().render_scale.chosen, None);
    assert_eq!(get(&lua), 0.7, "a reset brings the default back");
}

#[test]
fn match_display_follows_the_display_scale() {
    let (lua, (_, screen, app)) = vm();
    let scale = || {
        lua.load("return Video.GetDisplayScale()")
            .eval::<f32>()
            .unwrap()
    };
    assert_eq!(scale(), 1.0, "headless reads 1");
    lua.load(r#"Graphics.SetDefaultRenderScale("match_display")"#)
        .exec()
        .unwrap();
    assert_eq!(
        app.borrow().render_scale_default(),
        RenderScaleDefault::MatchDisplay
    );
    let read: String = lua
        .load("return Graphics.GetDefaultRenderScale()")
        .eval()
        .unwrap();
    assert_eq!(read, "match_display");
    assert_eq!(get(&lua), 1.0, "native at scale 1");
    screen.borrow_mut().set_display_scale(2.0);
    assert_eq!(
        (scale(), get(&lua)),
        (2.0, 0.5),
        "the 1x pixel count on a 2x display"
    );
}

#[test]
fn a_bad_default_is_an_error_and_changes_nothing() {
    let (lua, (_, _, app)) = vm();
    for bad in ["0", "-1", "'half'", "true", "nil"] {
        let set = format!("Graphics.SetDefaultRenderScale({bad})");
        let err = lua.load(&set).exec().unwrap_err().to_string();
        let said = format!(
            "Graphics.SetDefaultRenderScale: {} is not",
            bad.replace('\'', "\"")
        );
        assert!(err.contains(&said.replace("nil", "null")), "{bad}: {err}");
    }
    assert_eq!(
        app.borrow().render_scale_default(),
        RenderScaleDefault::default()
    );
}
