//! `Harness.Warnings` reads the run's warnings and errors (#2054).

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use crate::harness::Harness;

/// Run `scenario` on a fresh harness world (the default scene) with the API and
/// the `Harness` table, as `play` registers them.
fn run(scenario: &str) -> mlua::Result<()> {
    rusty_logging::init_for_tests();
    let out = rusty_test_support::temp::dir().join(format!("bridge_warn_{}", std::process::id()));
    let harness = Rc::new(RefCell::new(Harness::new(out, "")));
    let world = Rc::clone(&harness.borrow().world);
    let cells = world.borrow().script_manager().api_cells();
    let lua = Lua::new();
    cells.install(&lua).map_err(mlua::Error::RuntimeError)?;
    super::attempt(|| {
        super::super::register(&lua, &harness)?;
        lua.load(scenario).exec()
    })
}

#[test]
fn a_clean_step_run_has_no_warnings() {
    run(r#"
        Harness.Step(5)
        local w = Harness.Warnings()
        assert(type(w) == "table" and #w == 0, "a clean run: " .. tostring(w[1] and w[1].text))
    "#)
    .unwrap();
}

#[test]
fn a_scenarios_warning_and_error_show_with_level_and_text() {
    run(r#"
        Harness.Step(1)
        Debug.Warn("low ammo")
        Debug.Error("no weapon")
        local w = Harness.Warnings()
        assert(#w == 2, "two lines, got " .. #w)
        assert(w[1].level == "warning" and w[1].text:find("low ammo", 1, true), w[1].text)
        assert(w[2].level == "error" and w[2].text:find("no weapon", 1, true), w[2].text)
        local later = Harness.Warnings(1)
        assert(#later == 1 and later[1].level == "error", "after skips the earlier one")
        assert(#Harness.Warnings(2) == 0 and #Harness.Warnings(9) == 0, "nothing after the end")
        Debug.Log("info is not a warning")
        assert(#Harness.Warnings() == 2, "info lines are left out")
    "#)
    .unwrap();
}

#[test]
fn an_engine_warning_raised_in_a_call_shows_up() {
    run(r#"
        Harness.Step(1)
        local p = Scene.FindEntityByName("Player")
        assert(not Ragdoll.HitReact(p, p, 0, 0, 0, 0, 0, 0), "no ragdoll on the player")
        local w = Harness.Warnings()
        assert(#w == 1 and w[1].level == "warning", "one engine warning, got " .. #w)
        assert(w[1].text:find("[Ragdoll] HitReact", 1, true), w[1].text)
    "#)
    .unwrap();
}

#[test]
fn a_negative_after_is_refused() {
    let err = run("Harness.Warnings(-1)").unwrap_err().to_string();
    assert!(err.contains("after must be a whole 0 or more"), "{err}");
}
