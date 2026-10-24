//! A scenario's `Scene.GetScript` handle pins its one instance (#1384).

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use crate::dev::harness::{Harness, FIXED_DT};

const GET: &str =
    r#"return Scene.GetScript(Scene.FindEntityByName("Player"), "player_controller")"#;

/// Restarting Play makes a fresh instance under the same entity id and script
/// name: the old handle errors rather than following it, and a new lookup finds it.
#[test]
fn a_handle_errors_once_its_instance_is_replaced() {
    let out = crate::test_temp::dir().join(format!("bridge_pin_{}", std::process::id()));
    let harness = Rc::new(RefCell::new(Harness::new(out, "")));
    let world = Rc::clone(&harness.borrow().world);
    let cells = world.borrow().script_manager().api_cells();
    let ctx = cells.ctx();
    let lua = Lua::new();
    let restart = || {
        let mut w = world.borrow_mut();
        w.set_playing(false);
        w.tick(FIXED_DT);
        w.set_playing(true);
        w.tick(FIXED_DT);
    };
    lua.scope(|scope| {
        crate::api::register(&lua, scope, &ctx).map_err(mlua::Error::RuntimeError)?;
        super::register(&lua, &harness)?;
        lua.load("Harness.Step(2)").exec()?;
        let old: mlua::Value = lua.load(GET).eval()?;
        lua.globals().set("old", old)?;
        lua.load("old.pinned = 1").exec()?;
        restart();
        let err = lua.load("return old.vy").exec().unwrap_err();
        assert!(err.to_string().contains("is gone"), "{err}");
        let fresh: Option<i64> = lua.load(format!("{GET}.pinned")).eval()?;
        assert_eq!(fresh, None, "a new lookup finds the new instance");
        Ok(())
    })
    .unwrap();
}
