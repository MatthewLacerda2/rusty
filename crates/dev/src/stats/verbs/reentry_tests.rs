use mlua::Lua;

use super::super::wrap::wrap_globals;
use super::super::{advance, drive, take, Frame, RECORDER, SYSTEM, UPDATE_SYSTEM};
use super::{assert_ms, verb, vm};

const SPEND: &str = "spend(50)";

fn reenter_then_work(handler: &str) {
    let lua = vm(true);
    let then = lua
        .create_function(|_, f: mlua::Function| {
            f.call::<()>(())?;
            advance(20);
            Ok(())
        })
        .unwrap();
    let fake: mlua::Table = lua.globals().get("Fake").unwrap();
    fake.set("Then", then).unwrap();
    wrap_globals(&lua).unwrap();
    drive();
    take();
    SYSTEM.with(|s| s.set(UPDATE_SYSTEM));
    lua.load(format!("Fake.Then({handler})")).exec().unwrap();
    let frame = take();
    SYSTEM.with(|s| s.set(""));
    let then = verb(&frame.verbs, "Fake.Then").expect("Then is timed");
    assert_ms(then.ms, 20.0, "the verb's work after the handler, alone");
    assert_ms(
        frame.update_ms,
        20.0,
        "no other verb holds the handler's Lua",
    );
    assert_ms(frame.overhead_ms, 0.0, "nor do the wrappers");
    RECORDER.with_borrow(|r| assert!(r.stack.is_empty(), "nothing left open"));
}

#[test]
fn an_error_a_handler_catches_still_ends_the_lua_it_re_entered() {
    reenter_then_work(&format!(
        "function() pcall(function() error('x') end) {SPEND} end"
    ));
}

#[test]
fn a_main_chunk_a_verb_calls_back_is_lua_too() {
    reenter_then_work(&format!("load([[{SPEND}]])"));
}

#[test]
fn a_rust_half_in_the_halves_table_is_timed_as_the_verb_it_serves() {
    let lua = Lua::new();
    let half = lua.create_function(|_, n: f64| Ok(n * 2.0)).unwrap();
    let ns = lua.create_table().unwrap();
    ns.set("Each", half.clone()).unwrap();
    ns.set("Each/edit", half).unwrap();
    let all = lua.create_table().unwrap();
    all.set("Live", ns).unwrap();
    let key = rusty_script::api::RUST_HALVES;
    lua.set_named_registry_value(key, &all).unwrap();
    wrap_globals(&lua).unwrap();
    take();
    let call = lua
        .create_function(move |lua, ()| {
            let all: mlua::Table = lua.named_registry_value(key)?;
            let ns: mlua::Table = all.get("Live")?;
            let a: f64 = ns.get::<mlua::Function>("Each")?.call(1.0)?;
            let b: f64 = ns.get::<mlua::Function>("Each/edit")?.call(a)?;
            Ok(b)
        })
        .unwrap();
    assert_eq!(
        call.call::<f64>(()).unwrap(),
        4.0,
        "the halves still answer"
    );
    let frame = take();
    let each = verb(&frame.verbs, "Live.Each").map(|v| v.calls);
    assert_eq!(
        each,
        Some(2),
        "both halves under one row: {:?}",
        frame.verbs
    );
}

#[test]
fn the_wrappers_own_time_is_kept_apart_and_a_bare_vm_has_none() {
    let lua = vm(true);
    take();
    lua.load("for i = 1, 2000 do Fake.Cheap(i) end")
        .exec()
        .unwrap();
    assert_eq!(take().overhead_ms, 0.0);
    SYSTEM.with(|s| s.set(UPDATE_SYSTEM));
    lua.load("for i = 1, 2000 do Fake.Cheap(i) end")
        .exec()
        .unwrap();
    let frame = take();
    SYSTEM.with(|s| s.set(""));
    assert!(frame.overhead_ms > 0.0, "{frame:?}");
    let bare = vm(false);
    bare.load("Fake.Cheap(1)").exec().unwrap();
    assert_eq!(take(), Frame::default());
}

#[path = "particles_tests.rs"]
mod particles_tests;
