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

/// A table nested past `copy::MAX_DEPTH` is a Lua error, not a stack overflow;
/// one just inside the cap still crosses.
#[test]
fn a_too_deep_table_is_refused_not_overflowed() {
    let (from, into) = (Lua::new(), Lua::new());
    let nest = |depth: usize| -> mlua::Value {
        let code = format!("local t = {{}} for _ = 2, {depth} do t = {{t}} end return t");
        from.load(code).eval().unwrap()
    };
    let deep = super::copy::copy(&nest(super::copy::MAX_DEPTH + 1), &into).unwrap_err();
    assert!(deep.to_string().contains("nested deeper"), "{deep}");
    assert!(super::copy::copy(&nest(super::copy::MAX_DEPTH), &into).is_ok());
}
