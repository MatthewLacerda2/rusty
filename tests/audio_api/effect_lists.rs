//! A group-effect list must be a list (#1748): a forgotten outer brace, a hole, a
//! table nested past an EQ's `bands`, or a table that holds itself is a Lua error
//! naming the field, never a shorter list (and never a stack overflow).

use super::{fixture, register};
use mlua::Lua;

fn lua() -> Lua {
    let lua = Lua::new();
    register(&lua, &fixture());
    lua
}

fn error(lua: &Lua, script: &str) -> String {
    match lua.load(script).exec() {
        Ok(()) => panic!("{script}: no error"),
        Err(e) => e.to_string(),
    }
}

fn master_effects(lua: &Lua) -> usize {
    lua.load("return #Audio.GetGroupEffects('Master')")
        .eval()
        .unwrap()
}

#[test]
fn a_table_that_is_not_a_list_is_refused_and_changes_nothing() {
    let lua = lua();
    for (script, says) in [
        (
            "Audio.SetGroupEffects('Master', { kind = 'limiter', ceiling = -1 })",
            "'effects' must be a list",
        ),
        (
            "Audio.SetGroupEffects('Master', { { kind = 'limiter' }, nil, { kind = 'eq' } })",
            "'effects' has a hole",
        ),
        (
            "Audio.SetGroupEffects('SFX', { { kind = 'eq', bands = { shape = 'peak', gain = 3 } } })",
            "'bands' must be a list",
        ),
        (
            "Audio.SetGroupEffects('SFX', { { kind = 'eq', bands = { { shape = 'peak', q = {} } } } })",
            "'q' cannot be a table",
        ),
        (
            "Audio.SetGroupEffects('SFX', { 'limiter' })",
            "entry 1 must be a table",
        ),
        (
            "Audio.DefineSnapshot('X', { Master = { effects = { kind = 'limiter', ceiling = -6 } } })",
            "'effects' must be a list",
        ),
    ] {
        let e = error(&lua, script);
        assert!(e.contains(says), "{script}: {e}");
    }
    assert_eq!(master_effects(&lua), 1, "Master keeps its limiter");
}

#[test]
fn a_table_that_holds_itself_is_an_error_not_an_overflow() {
    let lua = lua();
    let script = "local t = { kind = 'eq' }; t.bands = { t }; Audio.SetGroupEffects('SFX', { t })";
    assert!(error(&lua, script).contains("cannot be a table"));
}

#[test]
fn a_snapshot_cannot_clear_a_sidechain_with_an_empty_name() {
    let lua = lua();
    lua.load("Audio.SetGroupEffects('Music', { { kind = 'compressor', sidechain = 'SFX' } })")
        .exec()
        .unwrap();
    let script = "Audio.DefineSnapshot('X', { Music = { effects = { { kind = 'compressor', sidechain = '' } } } })";
    assert!(error(&lua, script).contains("cannot change a sidechain"));
}
