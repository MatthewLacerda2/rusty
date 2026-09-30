//! The gamepad half of `Input` (#471) against a bare `InputState`: injected axes and
//! pad buttons reach Lua with per-tick edges, and the pad records (connection, dead
//! zones, rumble) read back what was written.

use std::cell::RefCell;

use mlua::Lua;
use rusty::core::gamepad::Rumble;
use rusty::core::input::InputState;

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

fn eval<T: for<'l> mlua::FromLuaMulti<'l>>(lua: &Lua, code: &str) -> T {
    lua.load(code).eval().unwrap()
}

#[test]
fn injected_axes_reach_lua_on_the_next_tick_and_hold() {
    let input = RefCell::new(InputState::new());
    with_input(&input, |lua| {
        lua.load(r#"Input.SetAxis("PadLeftX", 0.5); Input.SetAxis("Pad1RightTrigger", 2)"#)
            .exec()
            .unwrap();
        assert_eq!(eval::<f32>(lua, r#"return Input.GetAxis("PadLeftX")"#), 0.0);
        input.borrow_mut().begin_tick();
        let now: (f32, f32) = eval(
            lua,
            r#"return Input.GetAxis("PADLEFTX"), Input.GetAxis("pad1righttrigger")"#,
        );
        assert_eq!(now, (0.5, 1.0), "exact, and clamped to 1");
        input.borrow_mut().begin_tick();
        assert_eq!(eval::<f32>(lua, r#"return Input.GetAxis("PadLeftX")"#), 0.5);
    });
}

#[test]
fn pad_buttons_are_keys_with_one_tick_edges() {
    let input = RefCell::new(InputState::new());
    with_input(&input, |lua| {
        lua.load(r#"Input.Press("PadA")"#).exec().unwrap();
        input.borrow_mut().begin_tick();
        let edge = r#"return Input.GetKeyDown("PadA"), Input.IsKeyDown("PADA")"#;
        assert_eq!(eval::<(bool, bool)>(lua, edge), (true, true));
        input.borrow_mut().begin_tick();
        assert_eq!(eval::<(bool, bool)>(lua, edge), (false, true));
    });
}

#[test]
fn connection_dead_zones_and_rumble_read_back() {
    let input = RefCell::new(InputState::new());
    with_input(&input, |lua| {
        lua.load(
            r#"Input.SetPadConnected(1, true)
               Input.SetDeadZone(0.25, 0.1)
               Input.SetRumble(1, 0.8, 0.2, 0.5)"#,
        )
        .exec()
        .unwrap();
        let pads: (bool, bool) = eval(
            lua,
            "return Input.IsPadConnected(0), Input.IsPadConnected(1)",
        );
        assert_eq!(pads, (false, true));
        let zones: (f32, f32) = eval(lua, "return Input.GetDeadZone()");
        assert_eq!(zones, (0.25, 0.1));
        let rumble: (f32, f32, f32) = eval(lua, "return Input.GetRumble(1)");
        assert_eq!(rumble, (0.8, 0.2, 0.5));
    });
    let queued = input.borrow_mut().pads.take_rumble();
    assert_eq!(
        queued,
        vec![(1, Rumble::new(0.8, 0.2, 0.5))],
        "for the platform"
    );
}
