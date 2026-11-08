//! A scenario's `Damage.*` reaches the gameplay VM's `OnTakeDamage` (#1893).

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use crate::dev::harness::Harness;

#[test]
fn a_scenarios_damage_reaches_the_game_scripts_handler() {
    let out = crate::test_temp::dir().join(format!("bridge_damage_{}", std::process::id()));
    let harness = Rc::new(RefCell::new(Harness::new(out, "assets/scripts/bot.lua")));
    let cells = harness.borrow().world.borrow().script_manager().api_cells();
    let lua = Lua::new();
    cells.install(&lua).expect("cells install");
    crate::api::attempt(|| {
        super::super::register(&lua, &harness)?;
        lua.load(
            r#"
            Harness.Step(2)
            local bot = Scene.FindEntityByName("Enemy_1")
            assert(Damage.Apply(bot, 30, { type = "bullet" }) == 30, "the bot took it")
            assert(Scene.GetScript(bot, "bot").health == 70, "its handler ran")
            local x, y, z = Transform.GetPosition(bot)
            local hits = Damage.ApplyRadial(x, y, z, 10, { outer = 2 })
            assert(hits[1].id == bot and hits[1].amount == 10, "the list crossed back")
            local ok, err = pcall(Damage.ApplyRadial, x, y, z, 10, {})
            assert(not ok and tostring(err):find("opts.outer is required", 1, true), err)
            "#,
        )
        .exec()
    })
    .expect("the scenario runs");
}
