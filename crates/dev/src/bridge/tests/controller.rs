//! A scenario's `CharacterController.Move` reaches the gameplay VM's
//! `OnControllerColliderHit` (#1879).

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use crate::harness::Harness;

#[test]
fn a_scenarios_move_reaches_the_game_scripts_handler() {
    let dir = rusty_test_support::temp::dir();
    let listener = dir.join("listener.lua");
    let code = "local L = { hits = 0 }\n\
                function L.OnControllerColliderHit(id, hit) L.hits = L.hits + 1 end\n\
                return L";
    std::fs::write(&listener, code).unwrap();
    let out = dir.join(format!("bridge_controller_{}", std::process::id()));
    let harness = Rc::new(RefCell::new(Harness::new(out, "assets/scripts/bot.lua")));
    let cells = harness.borrow().world.borrow().script_manager().api_cells();
    let lua = Lua::new();
    cells.install(&lua).expect("cells install");
    lua.globals()
        .set("LISTENER", listener.to_string_lossy().into_owned())
        .unwrap();
    super::attempt(|| {
        super::super::register(&lua, &harness)?;
        lua.load(
            r#"
            local player = Scene.FindEntityByName("Player")
            assert(Scene.AddScript(player, LISTENER), "attached")
            Harness.Step(2)
            local below = CharacterController.Move(player, 0, -5, 0)
            assert(below, "the returns crossed back: landed on the floor")
            local hits = Scene.GetScript(player, "listener").hits
            assert(hits >= 1, "the handler ran: " .. tostring(hits))
            "#,
        )
        .exec()
    })
    .expect("the scenario runs");
}

#[test]
fn a_move_before_the_first_step_runs_in_the_scenario() {
    let dir = rusty_test_support::temp::dir();
    let out = dir.join(format!("bridge_controller_edit_{}", std::process::id()));
    let harness = Rc::new(RefCell::new(Harness::new(out, "assets/scripts/bot.lua")));
    let cells = harness.borrow().world.borrow().script_manager().api_cells();
    let lua = Lua::new();
    cells.install(&lua).expect("cells install");
    super::attempt(|| {
        super::super::register(&lua, &harness)?;
        lua.load(
            r#"
            local player = Scene.FindEntityByName("Player")
            local before = Transform.GetPosition(player)
            CharacterController.Move(player, 1, 0, 0)
            local after = Transform.GetPosition(player)
            assert(math.abs(after - before - 1) < 1e-4, "moved in edit mode")
            "#,
        )
        .exec()
    })
    .expect("the scenario's own Move ran: the game is not up yet");
}
