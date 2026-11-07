//! The upscaler (#1848): `Graphics.Set/GetUpscaler` write the player's choice in
//! the live graphics cell the renderer reads; with none, the project's default
//! runs (`Get/SetDefaultUpscaler`); a name that is no upscaler raises an error.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::api::bind;
use rusty::core::application::Application;
use rusty::core::settings::upscaler::Upscaler;
use rusty::core::settings::GraphicsSettings;
use rusty::scene::Scene;

/// A VM with `Graphics` over fresh cells.
fn vm() -> (Lua, Rc<RefCell<GraphicsSettings>>, Rc<RefCell<Application>>) {
    let (lua, scene) = (Lua::new(), Rc::new(RefCell::new(Scene::new())));
    let graphics = Rc::new(RefCell::new(GraphicsSettings::default()));
    let (screen, app): (Rc<RefCell<rusty::ui::ScreenSize>>, _) = (Rc::default(), Rc::default());
    let cells = (bind(&lua, &graphics), bind(&lua, &screen));
    rusty::api::graphics::register(&lua, bind(&lua, &scene), cells, bind(&lua, &app)).unwrap();
    (lua, graphics, app)
}

fn get(lua: &Lua) -> String {
    lua.load("return Graphics.GetUpscaler()").eval().unwrap()
}

#[test]
fn the_choice_writes_the_cell_and_the_default_runs_without_one() {
    let (lua, graphics, app) = vm();
    assert_eq!(get(&lua), "taa", "TAA's by default");
    lua.load(r#"Graphics.SetDefaultUpscaler("fsr2")"#)
        .exec()
        .unwrap();
    assert_eq!(app.borrow().upscaler_default(), Upscaler::Fsr2);
    assert_eq!(get(&lua), "fsr2", "no choice: the default runs at once");
    lua.load(r#"Graphics.SetUpscaler("taa")"#).exec().unwrap();
    assert_eq!(graphics.borrow().upscaler.chosen, Some(Upscaler::Taa));
    assert_eq!(get(&lua), "taa", "the player's choice wins");
    lua.load("Graphics.ResetUpscaler()").exec().unwrap();
    assert_eq!(graphics.borrow().upscaler.chosen, None);
    let default: String = lua
        .load("return Graphics.GetDefaultUpscaler()")
        .eval()
        .unwrap();
    assert_eq!((get(&lua), default), ("fsr2".into(), "fsr2".into()));
}

#[test]
fn a_name_that_is_no_upscaler_raises_and_changes_nothing() {
    let (lua, graphics, app) = vm();
    for verb in ["SetUpscaler", "SetDefaultUpscaler"] {
        let err = lua
            .load(format!(r#"Graphics.{verb}("dlss")"#))
            .exec()
            .unwrap_err();
        assert!(err.to_string().contains("not an upscaler"), "{verb}: {err}");
    }
    assert_eq!(graphics.borrow().upscaler, Default::default());
    assert_eq!(app.borrow().upscaler_default(), Upscaler::Taa);
}
