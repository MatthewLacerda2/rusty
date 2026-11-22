mod calls;
mod controller;
mod damage;
mod warnings;

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use crate::harness::{Harness, FIXED_DT};

const GET: &str =
    r#"return Scene.GetScript(Scene.FindEntityByName("Player"), "player_controller")"#;

#[test]
fn a_handle_errors_once_its_instance_is_replaced() {
    let out = rusty_test_support::temp::dir().join(format!("bridge_pin_{}", std::process::id()));
    let harness = Rc::new(RefCell::new(Harness::new(out, "")));
    let world = Rc::clone(&harness.borrow().world);
    let cells = world.borrow().script_manager().api_cells();
    let lua = Lua::new();
    let restart = || {
        let mut w = world.borrow_mut();
        w.set_playing(false);
        w.tick(FIXED_DT);
        w.set_playing(true);
        w.tick(FIXED_DT);
    };
    cells.install(&lua).unwrap();
    attempt(|| {
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

fn attempt<R>(body: impl FnOnce() -> mlua::Result<R>) -> mlua::Result<R> {
    body()
}
