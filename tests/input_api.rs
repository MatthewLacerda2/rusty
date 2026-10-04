//! The `Input` scripting namespace (#416) against a bare `InputState`: every read,
//! the cursor request and the injection verbs, with ticks driven by `begin_tick`
//! exactly as `GameWorld::tick` drives them.

use std::cell::RefCell;

use mlua::Lua;
use rusty::core::input::{CursorState, InputState};

/// Run `f` with a Lua VM whose `Input` table is bound to `input`.
fn with_input(input: &RefCell<InputState>, f: impl FnOnce(&Lua)) {
    let lua = Lua::new();
    lua.scope(|scope| {
        rusty::api::input::register_readable(&lua, scope, input).unwrap();
        rusty::api::input::register_writable(&lua, scope, input).unwrap();
        f(&lua);
        Ok(())
    })
    .unwrap();
}

fn eval<T: mlua::FromLuaMulti>(lua: &Lua, code: &str) -> T {
    lua.load(code).eval().unwrap()
}

#[test]
fn key_and_mouse_button_edges_hold_for_one_tick() {
    let input = RefCell::new(InputState::new());
    with_input(&input, |lua| {
        lua.load(r#"Input.Press("Mouse0"); Input.Press("e")"#)
            .exec()
            .unwrap();
        input.borrow_mut().begin_tick();
        let now: (bool, bool, bool) = eval(
            lua,
            r#"return Input.GetKeyDown("MOUSE0"), Input.IsKeyDown("Mouse0"), Input.GetKeyDown("E")"#,
        );
        assert_eq!(now, (true, true, true));

        input.borrow_mut().begin_tick();
        assert!(!eval::<bool>(lua, r#"return Input.GetKeyDown("Mouse0")"#));

        lua.load(r#"Input.Release("Mouse0")"#).exec().unwrap();
        input.borrow_mut().begin_tick();
        assert!(eval::<bool>(lua, r#"return Input.GetKeyUp("Mouse0")"#));
    });
}

#[test]
fn mouse_scroll_and_text_injection_publish_on_the_next_tick() {
    let input = RefCell::new(InputState::new());
    with_input(&input, |lua| {
        lua.load(
            r#"Input.MoveMouse(320, 240)
               Input.AddMouseDelta(4, -2); Input.AddMouseDelta(1, 1)
               Input.Scroll(-1); Input.TypeText("ok")"#,
        )
        .exec()
        .unwrap();
        let pos: (f64, f64) = eval(lua, "return Input.GetMousePosition()");
        assert_eq!(pos, (320.0, 240.0), "the pointer moves at once");
        let delta: (f64, f64) = eval(lua, "return Input.GetMouseDelta()");
        assert_eq!(delta, (0.0, 0.0), "deltas wait for the tick");

        input.borrow_mut().begin_tick();
        let delta: (f64, f64) = eval(lua, "return Input.GetMouseDelta()");
        assert_eq!(delta, (5.0, -1.0));
        assert_eq!(eval::<f64>(lua, "return Input.GetScrollDelta()"), -1.0);
        assert_eq!(eval::<String>(lua, "return Input.GetTextInput()"), "ok");

        input.borrow_mut().begin_tick();
        assert_eq!(eval::<String>(lua, "return Input.GetTextInput()"), "");
    });
}

#[test]
fn the_cursor_request_is_recorded_for_the_platform() {
    let input = RefCell::new(InputState::new());
    input.borrow_mut().reset_cursor(CursorState::PLAY);
    with_input(&input, |lua| {
        let now: (bool, bool) = eval(
            lua,
            "return Input.IsCursorLocked(), Input.IsCursorVisible()",
        );
        assert_eq!(now, (true, false), "Play defaults to locked + hidden");
        lua.load("Input.SetCursorLocked(false); Input.SetCursorVisible(true)")
            .exec()
            .unwrap();
    });
    assert_eq!(input.borrow().cursor(), CursorState::FREE);
}
