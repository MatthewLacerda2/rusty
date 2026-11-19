//! Where re-entered Lua ends (an error a handler catches, a main chunk), the
//! Rust halves of a Lua-written verb, and the wrappers' own time.

use mlua::Lua;

use super::super::wrap::wrap_globals;
use super::super::{take, RECORDER};
use super::{verb, vm};

/// Spin the VM: a stretch of Lua long enough to tell from a cheap verb.
const SPIN: &str = "local x = 0 for i = 1, 2000000 do x = x + i % 7 end";

/// `Fake.Then(handler)`: a verb that calls `handler` back, then sleeps 20 ms on
/// Rust's side. Its own time, which must hold the sleep and none of the spin.
fn reenter_then_sleep(handler: &str) -> f64 {
    let lua = vm(true);
    // A Rust verb that runs a handler, then works (sleeps) after it returns.
    let after = lua
        .create_function(|_, f: mlua::Function| {
            f.call::<()>(())?;
            std::thread::sleep(std::time::Duration::from_millis(20));
            Ok(())
        })
        .unwrap();
    let fake: mlua::Table = lua.globals().get("Fake").unwrap();
    fake.set("Then", after).unwrap();
    wrap_globals(&lua).unwrap();
    take();
    lua.load(format!("Fake.Then({handler})")).exec().unwrap();
    let frame = take();
    let then = verb(&frame.verbs, "Fake.Then").expect("Then is timed");
    RECORDER.with_borrow(|r| assert!(r.stack.is_empty(), "nothing left open"));
    then.ms
}

#[test]
fn an_error_a_handler_catches_still_ends_the_lua_it_re_entered() {
    let handler = format!("function() pcall(function() error('x') end) {SPIN} end");
    let own = reenter_then_sleep(&handler);
    assert!(
        own >= 20.0,
        "the verb's work after the handler is its own: {own} ms"
    );
    assert!(own < 60.0, "and the handler's spin is not: {own} ms");
}

#[test]
fn a_main_chunk_a_verb_calls_back_is_lua_too() {
    let handler = format!("load([[{SPIN}]])");
    let own = reenter_then_sleep(&handler);
    assert!((20.0..60.0).contains(&own), "{own} ms");
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
    // Outside any system, so the overhead is not summed for update_scripts...
    assert_eq!(take().overhead_ms, 0.0);
    super::super::SYSTEM.with(|s| s.set(super::super::UPDATE_SYSTEM));
    lua.load("for i = 1, 2000 do Fake.Cheap(i) end")
        .exec()
        .unwrap();
    let frame = take();
    super::super::SYSTEM.with(|s| s.set(""));
    assert!(frame.overhead_ms > 0.0, "{frame:?}");
    let bare = vm(false);
    bare.load("Fake.Cheap(1)").exec().unwrap();
    assert_eq!(take(), super::super::Frame::default());
}

#[path = "particles_tests.rs"]
mod particles_tests;
