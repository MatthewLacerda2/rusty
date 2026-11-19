//! The verb timers on a bare VM: what they count, what they leave out, and that a
//! wrapped verb behaves as the bare one. `harness_tests` runs them on a scene.
//! A test about which stretch time is charged to [`drive`]s the clock and moves
//! it by known amounts, so it asserts exact charges, never a span of real time.

use mlua::{Function, Lua};

use super::wrap::wrap_globals;
use super::{advance, drive, take, Verb};

/// A VM with a `Fake` namespace: `Cheap`, `Work(ms)` (moves the driven clock),
/// `Fail` (a Rust error), `Reenter(f)` (calls `f` back from Rust, as `Move`
/// raises its hits) and `Lua` (a function written in Lua); a lowercase `fake`
/// table with a Rust function; and a global `spend(ms)`: Lua's own work, which
/// moves the driven clock outside any verb.
pub(super) fn vm(wrapped: bool) -> Lua {
    let lua = Lua::new();
    let fake = lua.create_table().unwrap();
    let cheap = lua.create_function(|_, n: f64| Ok(n + 1.0)).unwrap();
    fake.set("Cheap", cheap.clone()).unwrap();
    let work = lua.create_function(|_, ms: u64| {
        advance(ms);
        Ok(())
    });
    fake.set("Work", work.unwrap()).unwrap();
    let spend = lua.create_function(|_, ms: u64| {
        advance(ms);
        Ok(())
    });
    lua.globals().set("spend", spend.unwrap()).unwrap();
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

/// `got` is `want` ms, but for the float rounding of a sum of durations.
pub(super) fn assert_ms(got: f64, want: f64, what: &str) {
    assert!((got - want).abs() < 1e-9, "{what}: {got} ms, not {want}");
}

pub(super) fn verb<'a>(verbs: &'a [Verb], name: &str) -> Option<&'a Verb> {
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
    assert_eq!(keys, ["Cheap", "Fail", "Lua", "Reenter", "Work"]);
}

#[test]
fn lua_a_verb_re_enters_is_not_the_verbs_time_and_a_nested_verb_is_its_own() {
    let lua = vm(true);
    drive();
    take();
    // 7 ms of Lua inside `Reenter`, then a nested verb's 30 ms of work.
    lua.load("Fake.Reenter(function() spend(7) Fake.Work(30) end)")
        .exec()
        .unwrap();
    let frame = take();
    let outer = verb(&frame.verbs, "Fake.Reenter").expect("Reenter is timed");
    let work = verb(&frame.verbs, "Fake.Work").expect("Work is timed");
    assert_ms(work.ms, 30.0, "the nested verb holds its own work");
    assert_ms(
        outer.ms,
        0.0,
        "Reenter holds neither the Lua nor the nested verb",
    );
    assert_eq!((outer.calls, work.calls), (1, 1));
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
    assert!(bare[0].contains("no such entity 7"), "{}", bare[0]);
    // Raised in the verb: the same text, traceback and all.
    assert_eq!(bare[..2], wrapped[..2]);
    // Raised in Lua the verb called back: the same but for the bare verb's own
    // frame, which its wrapper calls, as one `[C]: in ?` line above the verb.
    for (bare, wrapped) in bare[2..].iter().zip(&wrapped[2..]) {
        let wrapped = wrapped.replacen("\t[C]: in ?\n\t[C]: in field", "\t[C]: in field", 1);
        let wrapped = wrapped.replacen("\t[C]: in ?\n\t>[C]: in field", "\t>[C]: in field", 1);
        assert_eq!(bare, &wrapped);
    }
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

#[path = "reentry_tests.rs"]
mod reentry_tests;
