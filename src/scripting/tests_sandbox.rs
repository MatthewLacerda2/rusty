//! The deterministic Lua sandbox (#443): no wall clock, one seeded RNG stream.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::render::Camera;
use crate::scene::Scene;
use crate::time::Time;

use super::console::ConsoleLogs;
use super::manager::ScriptManager;

fn live_manager() -> ScriptManager {
    let mut m = ScriptManager::new(
        Rc::new(RefCell::new(Scene::new())),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -5.0, 5.0, -5.0, 5.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
        Rc::new(RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0))),
        Rc::new(RefCell::new(Time::new())),
    );
    m.init_runtime(&Rc::new(RefCell::new(None))).expect("init");
    m
}

/// Run `code` and return the string it left in the global `out`.
fn run(m: &ScriptManager, code: &str) -> String {
    m.exec(code).expect("lua runs");
    let lua = m.lua.as_ref().expect("live");
    lua.globals().get("out").expect("`out` is a string")
}

/// Draws a mixed sequence through every entry point into one comparable string.
const DRAWS: &str = r#"
local t = {}
for _ = 1, 5 do
    t[#t + 1] = math.random(1, 100)
    t[#t + 1] = string.format("%.17g", math.random())
    t[#t + 1] = Random.Range(0, 10)
    t[#t + 1] = string.format("%.17g", Random.Range(-1.0, 1.0))
    local x, y, z = Random.OnUnitSphere()
    t[#t + 1] = string.format("%.9g,%.9g,%.9g", x, y, z)
end
out = table.concat(t, " ")
"#;

#[test]
fn wall_clock_and_host_access_are_absent() {
    let m = live_manager();
    let out = run(
        &m,
        "out = tostring(os) .. ' ' .. tostring(io) .. ' ' .. type(math.floor)",
    );
    assert_eq!(out, "nil nil function");
    assert!(
        m.exec("return os.time()").is_err(),
        "os.time must fail loudly"
    );
}

#[test]
fn two_fresh_runtimes_draw_identical_streams() {
    let a = run(&live_manager(), DRAWS);
    assert_eq!(a, run(&live_manager(), DRAWS));
}

#[test]
fn every_play_restarts_the_stream_from_the_default_seed() {
    let mut m = live_manager();
    let first = run(&m, DRAWS);
    assert_ne!(
        first,
        run(&m, DRAWS),
        "the stream advances within a session"
    );
    m.init_runtime(&Rc::new(RefCell::new(None)))
        .expect("re-init");
    assert_eq!(first, run(&m, DRAWS), "a new Play restarts it");
}

#[test]
fn set_seed_and_randomseed_restart_the_same_stream() {
    let m = live_manager();
    let code = r#"
        Random.SetSeed(42); local a = Random.Range(0, 1000000)
        math.randomseed(42); local b = Random.Range(0, 1000000)
        Random.SetSeed(43); local c = Random.Range(0, 1000000)
        out = tostring(a == b) .. " " .. tostring(a == c)
    "#;
    assert_eq!(run(&m, code), "true false");
}

#[test]
fn range_overloads_follow_the_number_subtype() {
    let m = live_manager();
    let code = r#"
        local ok = true
        for _ = 1, 500 do
            local i = Random.Range(1, 3)
            ok = ok and math.type(i) == "integer" and (i == 1 or i == 2)
            local f = Random.Range(1.0, 3)
            ok = ok and math.type(f) == "float" and f >= 1.0 and f < 3.0
            local d = math.random(6)
            ok = ok and d >= 1 and d <= 6
            local e = math.random(-2, 2)
            ok = ok and e >= -2 and e <= 2
        end
        ok = ok and Random.Range(5, 5) == 5 and math.random(3, 3) == 3
        ok = ok and not pcall(math.random, 3, 1) and not pcall(math.random, 1.5)
        out = tostring(ok)
    "#;
    assert_eq!(run(&m, code), "true");
}

#[test]
fn cached_math_random_survives_across_evaluations() {
    // `local random = math.random` at load time is a common Lua idiom; the shim
    // resolves `Random` per call, so it must keep working in a later evaluation.
    let m = live_manager();
    m.exec("cached = math.random").expect("cache");
    assert_eq!(run(&m, "local v = cached(1, 1); out = tostring(v)"), "1");
}
