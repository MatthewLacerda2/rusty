//! The verb timers on a bare VM: what they count, what they leave out, and that a
//! wrapped verb behaves as the bare one. `harness_tests` runs them on a scene.

use std::time::{Duration, Instant};

use mlua::{Function, Lua};

use super::wrap::wrap_globals;
use super::{take, Verb};

/// A VM with a `Fake` namespace: `Cheap`, `Sleep(ms)`, `Fail` (a Rust error),
/// `Reenter(f)` (calls `f` back from Rust, as `Move` raises its hits) and `Lua`
/// (a function written in Lua); and a lowercase `fake` table with a Rust function.
fn vm(wrapped: bool) -> Lua {
    let lua = Lua::new();
    let fake = lua.create_table().unwrap();
    let cheap = lua.create_function(|_, n: f64| Ok(n + 1.0)).unwrap();
    fake.set("Cheap", cheap.clone()).unwrap();
    let sleep = lua.create_function(|_, ms: u64| {
        std::thread::sleep(Duration::from_millis(ms));
        Ok(())
    });
    fake.set("Sleep", sleep.unwrap()).unwrap();
    let fail = lua.create_function(|_, ()| -> mlua::Result<()> {
        Err(mlua::Error::runtime("Fake.Fail: no such entity 7"))
    });
    fake.set("Fail", fail.unwrap()).unwrap();
    let reenter = lua.create_function(|_, f: Function| f.call::<mlua::MultiValue>(()));
    fake.set("Reenter", reenter.unwrap()).unwrap();
    let in_lua: Function = lua.load("function(x) return x end").eval().unwrap();
    fake.set("Lua", in_lua).unwrap();
    lua.globals().set("Fake", fake).unwrap();
    let lower = lua.create_table().unwrap();
    lower.set("cheap", cheap).unwrap();
    lua.globals().set("fake", lower).unwrap();
    if wrapped {
        wrap_globals(&lua).unwrap();
    }
    lua
}

fn verb<'a>(verbs: &'a [Verb], name: &str) -> Option<&'a Verb> {
    verbs.iter().find(|v| v.name == name)
}

#[test]
fn a_verb_called_n_times_counts_n_calls_and_returns_what_it_did() {
    let lua = vm(true);
    take();
    let sum: f64 = lua
        .load("local s = 0 for i = 1, 25 do s = s + Fake.Cheap(i) end return s")
        .eval()
        .unwrap();
    assert_eq!(sum, (1..=25).map(|i| f64::from(i) + 1.0).sum::<f64>());
    let frame = take();
    let cheap = verb(&frame.verbs, "Fake.Cheap").expect("Cheap is timed");
    assert_eq!(cheap.calls, 25);
    assert!(take().verbs.is_empty(), "a take starts afresh");
}

#[test]
fn only_the_namespaces_rust_functions_are_wrapped_and_none_is_renamed() {
    let lua = vm(true);
    take();
    lua.load("Fake.Lua(1) fake.cheap(1)").exec().unwrap();
    assert!(
        take().verbs.is_empty(),
        "a Lua function and a lowercase table are not verbs"
    );
    let keys: Vec<String> = lua
        .load("local k = {} for n in pairs(Fake) do k[#k + 1] = n end table.sort(k) return k")
        .eval()
        .unwrap();
    assert_eq!(keys, ["Cheap", "Fail", "Lua", "Reenter", "Sleep"]);
}

#[test]
fn lua_a_verb_re_enters_is_not_the_verbs_time_and_a_nested_verb_is_its_own() {
    let lua = vm(true);
    // Lua busy for a while inside `Reenter`, then a nested verb that sleeps.
    let spin = "local x = 0 for i = 1, 3000000 do x = x + i % 7 end";
    let started = Instant::now();
    lua.load(format!(
        "Fake.Reenter(function() {spin} Fake.Sleep(30) end)"
    ))
    .exec()
    .unwrap();
    let total = started.elapsed().as_secs_f64() * 1000.0;
    let frame = take();
    let outer = verb(&frame.verbs, "Fake.Reenter").expect("Reenter is timed");
    let sleep = verb(&frame.verbs, "Fake.Sleep").expect("Sleep is timed");
    assert!(
        sleep.ms >= 30.0,
        "the nested verb holds its own sleep: {sleep:?}"
    );
    let lua_ms = total - sleep.ms;
    assert!(lua_ms > 5.0, "the spin should take a while: {lua_ms} ms");
    assert!(
        outer.ms < lua_ms / 4.0,
        "Reenter's own time ({} ms) holds neither the re-entered Lua ({lua_ms} ms) \
         nor the nested verb",
        outer.ms
    );
    assert_eq!((outer.calls, sleep.calls), (1, 1));
}

#[test]
fn a_verb_called_inside_a_coroutine_is_timed_and_the_coroutine_still_yields() {
    let lua = vm(true);
    take();
    let got: f64 = lua
        .load(
            "local co = coroutine.wrap(function() local a = Fake.Cheap(1) \
             coroutine.yield(a) return Fake.Cheap(a) end) \
             return co() + co()",
        )
        .eval()
        .unwrap();
    assert_eq!(got, 2.0 + 3.0);
    let frame = take();
    assert_eq!(verb(&frame.verbs, "Fake.Cheap").map(|v| v.calls), Some(2));
}

/// Each chunk's error, as a script's console line would show it.
fn errors(lua: &Lua) -> Vec<String> {
    let chunks = [
        "Fake.Fail()",
        "Fake.Cheap('not a number')",
        "Fake.Reenter(function() error('the handler broke') end)",
        "Fake.Reenter(function() Fake.Fail() end)",
    ];
    let run = |c: &str| lua.load(c).set_name("=script.lua").exec().unwrap_err();
    chunks.iter().map(|c| run(c).to_string()).collect()
}

#[test]
fn a_wrapped_verbs_error_reads_as_the_bare_verbs() {
    let (bare, wrapped) = (errors(&vm(false)), errors(&vm(true)));
    assert_eq!(bare, wrapped);
    assert!(bare[0].contains("no such entity 7"), "{}", bare[0]);
}

#[test]
fn a_verb_left_by_an_error_closes_and_the_next_call_counts() {
    let lua = vm(true);
    take();
    let _ = lua
        .load("Fake.Reenter(function() local t = {} t.x.y = 1 end)")
        .exec();
    lua.load("Fake.Cheap(1)").exec().unwrap();
    let frame = take();
    assert_eq!(verb(&frame.verbs, "Fake.Reenter").map(|v| v.calls), Some(1));
    assert_eq!(verb(&frame.verbs, "Fake.Cheap").map(|v| v.calls), Some(1));
    super::RECORDER.with_borrow(|r| assert!(r.stack.is_empty(), "nothing left open"));
}

#[path = "harness_tests.rs"]
mod harness_tests;
