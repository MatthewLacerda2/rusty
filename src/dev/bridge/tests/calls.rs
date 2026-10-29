//! A scenario calls a live game script's functions through its handle (#1580).

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use crate::dev::harness::Harness;

/// Game-script functions put on the player controller's instance in the
/// gameplay VM: `self` fields, a Scene call (the API scope), a table return and
/// a gameplay error, loaded as the chunk `game_calls` so its frames are known.
const GAME: &str = r#"
local t = ...
function t:Refill(n) self.ammo = (self.ammo or 0) + n return self.ammo end
function t.Add(a, b) return a + b end
function t.Who() return Scene.FindEntityByName("Player") end
function t.Pack(list) return { n = #list, first = list[1] } end
function t.Boom() error("magazine jammed") end
function t.Same(a, b) return rawequal(a, b) end
function t.Twice() local x = {} return x, x end
function t:Leak() self.ammo = 99 return function() end end
"#;

/// Run `scenario` against a harness world whose player controller carries
/// [`GAME`]'s functions, with `ref` the scenario's handle on it.
fn run(scenario: &str) -> mlua::Result<()> {
    let out = crate::test_temp::dir().join(format!("bridge_calls_{}", std::process::id()));
    let harness = Rc::new(RefCell::new(Harness::new(out, "")));
    let world = Rc::clone(&harness.borrow().world);
    let cells = world.borrow().script_manager().api_cells();
    let ctx = cells.ctx();
    let lua = Lua::new();
    lua.scope(|scope| {
        crate::api::register(&lua, scope, &ctx).map_err(mlua::Error::RuntimeError)?;
        super::super::register(&lua, &harness)?;
        lua.load("Harness.Step(2)").exec()?;
        lua.load(r#"ref = Scene.GetScript(Scene.FindEntityByName("Player"), "player_controller")"#)
            .exec()?;
        let id: u32 = lua
            .load(r#"return Scene.FindEntityByName("Player")"#)
            .eval()?;
        world
            .borrow()
            .script_manager()
            .with_script(id, "player_controller", |glua, t| {
                glua.load(GAME)
                    .set_name("=game_calls")
                    .call::<()>(t.clone())
            })?;
        lua.load(scenario).exec()
    })
}

#[test]
fn a_scenario_calls_game_functions_with_and_without_self() {
    run(r#"
        assert(ref:Refill(30) == 30, "self is the instance")
        assert(ref.Refill(ref, 5) == 35, "explicit self too")
        assert(ref.ammo == 35, "the call wrote the game script's field")
        assert(ref.Add(2, 3) == 5, "plain arguments")
        assert(ref.Who() == Scene.FindEntityByName("Player"), "the game API is live in the call")
        local p = ref.Pack({ "rifle", "pistol" })
        assert(p.n == 2 and p.first == "rifle", "a table return is copied back")
        local refill = ref.Refill
        assert(refill(ref, 1) == 36, "a proxy read once calls later")
    "#)
    .unwrap();
}

#[test]
fn a_call_copies_its_arguments_and_returns_once_each() {
    run(r#"
        local t = {}
        assert(ref.Same(t, t), "a table passed twice arrives as one")
        local a, b = ref.Twice()
        assert(rawequal(a, b), "a table returned twice comes back as one")
    "#)
    .unwrap();
}

#[test]
fn a_game_error_carries_the_game_scripts_traceback() {
    let err = run("ref.Boom()").unwrap_err().to_string();
    assert!(
        err.contains("Boom failed") && err.contains("magazine jammed"),
        "{err}"
    );
    // A frame only the gameplay side has: the game chunk's `Boom`, line 7.
    assert!(err.contains("game_calls:7: in "), "{err}");
}

#[test]
fn a_function_cannot_cross_either_way() {
    let err = run("ref.Add(function() end, 1)").unwrap_err().to_string();
    assert!(
        err.contains("function (or a table holding one) cannot cross"),
        "{err}"
    );
    // The return fails after the call ran: its write stands.
    let err = run(r#"
        local ok, e = pcall(ref.Leak, ref)
        assert(not ok and ref.ammo == 99, "the call ran")
        error(e)
    "#)
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("Leak returned a value that cannot cross back; the call itself ran"),
        "{err}"
    );
}
