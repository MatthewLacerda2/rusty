//! The VM that reads a script's table for Add Component and the inspector.

use std::path::Path;

use mlua::Value;

use crate::scripting::discovery::offered;
use crate::scripting::sandbox::{inspection_vm, sandboxed_vm, WITHHELD};
use crate::scripting::schema::read_fields;
use rusty_components::ScriptFieldValue;

const LIFECYCLE: &str = "return { Start = function() end }";

#[test]
fn the_inspection_vm_withholds_what_play_withholds() {
    let inspection = inspection_vm(Path::new("")).unwrap();
    let play = sandboxed_vm().unwrap();
    for (lib, _) in WITHHELD {
        assert!(
            inspection.globals().get::<Value>(lib).unwrap().is_nil(),
            "{lib}"
        );
        assert!(play.globals().get::<Value>(lib).unwrap().is_nil(), "{lib}");
    }
    assert!(!inspection
        .globals()
        .get::<Value>("Transform")
        .unwrap()
        .is_nil());
}

#[test]
fn a_chunk_reaching_for_a_withheld_library_at_load_is_not_offered_and_says_why() {
    for (lib, _) in WITHHELD {
        let why = offered("a.lua", &format!("local _ = {lib}.x\n{LIFECYCLE}")).unwrap_err();
        assert!(why.contains("attempt to index a nil value"), "{why}");
    }
    let inside_start = "return { Start = function() os.getenv('x') end }";
    assert_eq!(offered("a.lua", inside_start), Ok(true));
}

#[test]
fn a_script_reaching_for_os_at_load_has_no_fields() {
    let code = "os.getenv('x')\nreturn { fields = { speed = 2 } }";
    assert!(read_fields(Path::new(""), "a.lua", code).is_err());
}

#[test]
fn an_error_names_the_script_and_its_line() {
    let code = "local a = 1\n\nos.getenv('x')\nreturn {}";
    let why = offered("assets/scripts/bad.lua", code).unwrap_err();
    assert!(why.contains("assets/scripts/bad.lua:3:"), "{why}");
    let why = read_fields(Path::new(""), "assets/scripts/bad.lua", code).unwrap_err();
    assert!(
        why.to_string().contains("assets/scripts/bad.lua:3:"),
        "{why}"
    );
}

/// Held against a plain VM seeded 0, not a second inspection: two VMs built in a row share
/// the clock's second and often an address, so an unseeded draw could repeat.
#[test]
fn a_field_default_drawn_from_math_random_is_the_same_every_run() {
    let code = "math.randomseed()\nreturn { fields = { a = math.random(1, 1000000), \
                b = math.random() } }";
    let fields = read_fields(Path::new(""), "r.lua", code).unwrap();
    let defaults: Vec<_> = fields.into_iter().map(|f| f.default).collect();
    let (a, b): (f64, f64) = mlua::Lua::new()
        .load("math.randomseed(0)\nreturn math.random(1, 1000000), math.random()")
        .eval()
        .unwrap();
    let pinned = [ScriptFieldValue::Number(a), ScriptFieldValue::Number(b)];
    assert_eq!(defaults, pinned);
}

#[test]
fn a_script_that_runs_in_play_is_not_reported_as_failing() {
    let extends = "local Base = require('base')\nlocal M = Base.extend{ speed = 1 }\n\
                   function M.Start() end\nreturn M";
    assert_eq!(offered("e.lua", extends), Ok(true));
    let compares = format!("if Time.time > 0 or Time.time <= 0 then end\n{LIFECYCLE}");
    assert_eq!(offered("c.lua", &compares), Ok(true));
}

#[test]
fn what_really_cannot_load_still_fails() {
    assert!(offered("s.lua", "this is not lua").is_err());
    assert!(offered("d.lua", &format!("dofile('no/such/file.lua')\n{LIFECYCLE}")).is_err());
}

#[test]
fn dofile_and_loadfile_resolve_against_the_workspace_as_in_play() {
    let root = rusty_test_support::temp::dir();
    std::fs::write(root.join("speed.lua"), "return 7").unwrap();
    let code = "return { fields = { a = dofile('speed.lua'), b = loadfile('speed.lua')() } }";
    let fields = read_fields(&root, "w.lua", code).unwrap();
    let defaults: Vec<_> = fields.into_iter().map(|f| f.default).collect();
    let seven = ScriptFieldValue::Number(7.0);
    assert_eq!(defaults, [seven.clone(), seven]);
}
