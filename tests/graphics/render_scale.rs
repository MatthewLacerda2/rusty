//! The render scale (#1498): `Graphics.Set/GetRenderScale` write the live graphics
//! cell the renderer reads, clamped to URP's range, native by default.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::api::bind;
use rusty::core::settings::render_scale::RenderScale;
use rusty::core::settings::GraphicsSettings;
use rusty::scene::Scene;

#[test]
fn set_render_scale_writes_the_cell_clamped_and_reads_back() {
    let (lua, scene) = (Lua::new(), Rc::new(RefCell::new(Scene::new())));
    let (graphics, app) = (
        Rc::new(RefCell::new(GraphicsSettings::default())),
        Rc::default(),
    );
    rusty::api::graphics::register(
        &lua,
        bind(&lua, &scene),
        bind(&lua, &graphics),
        bind(&lua, &app),
    )
    .unwrap();
    let get = || lua.load("return Graphics.GetRenderScale()").eval::<f32>();
    assert_eq!(get().unwrap(), 1.0, "native by default");
    lua.load("Graphics.SetRenderScale(0.5)").exec().unwrap();
    assert_eq!(graphics.borrow().render_scale, RenderScale::new(0.5));
    assert_eq!(get().unwrap(), 0.5);
    lua.load("Graphics.SetRenderScale(9)").exec().unwrap();
    assert_eq!(get().unwrap(), RenderScale::MAX);
    lua.load("Graphics.SetRenderScale(0)").exec().unwrap();
    assert_eq!(get().unwrap(), RenderScale::MIN);
}
