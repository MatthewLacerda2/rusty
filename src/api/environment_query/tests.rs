//! `EnvironmentQuery` through Lua (#1892): the documented returns, errors that
//! name their field, Unreal's edge behaviour, and the one random draw.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use mlua::{Lua, Table};

use super::register;
use crate::ai::query::test_world::Arena;
use crate::ai::query::QueryQueue;
use crate::api::{bind, ApiCtx};
use crate::core::random::Random;
use crate::core::settings::video::VideoSettings;
use crate::ui::ScreenSize;

/// A VM over an open arena, with a querier `bot` and a `player` entity bound as
/// globals; the seeded `Random` cell for checking draws.
pub(super) fn vm() -> (Lua, Rc<RefCell<Random>>) {
    let mut arena = Arena::new(|_| {});
    let bot = arena.agent(Vec3::new(-6.0, 0.0, 0.0));
    let player = arena.scene.add_entity("Player".to_string());
    let lua = Lua::new();
    let rng = Rc::new(RefCell::new(Random::new(9)));
    bind(&lua, &Rc::new(RefCell::new(arena.scene)));
    bind(&lua, &Rc::new(RefCell::new(arena.nav)));
    bind(&lua, &Rc::new(RefCell::new(Some(arena.physics))));
    bind(&lua, &Rc::new(RefCell::new(arena.camera)));
    bind(&lua, &Rc::new(RefCell::new(ScreenSize::default())));
    bind(&lua, &Rc::new(RefCell::new(VideoSettings::default())));
    bind(&lua, &rng);
    bind(&lua, &Rc::<RefCell<QueryQueue>>::default());
    register(&lua, &ApiCtx::default()).unwrap();
    lua.globals().set("bot", bot).unwrap();
    lua.globals().set("player", player).unwrap();
    (lua, rng)
}

pub(super) const RING: &str = r#"
    spec = { generator = { donut = { context = "target", inner = 3, outer = 3, per_ring = 8 } },
             tests = { { path = "exists" },
                       { distance = { to = "querier" }, score = { curve = "inverse" } } } }"#;

pub(super) fn error(lua: &Lua, code: &str) -> String {
    let err = lua.load(code).exec().expect_err("an error");
    err.to_string()
}

#[test]
fn run_returns_the_best_spot_and_run_all_the_ranked_list() {
    let (lua, _) = vm();
    lua.load(RING).exec().unwrap();
    let (found, x, z, score): (bool, f32, f32, f32) = lua
        .load("local f, x, y, z, s = EnvironmentQuery.Run(bot, spec, { target = player }); return f, x, z, s")
        .eval()
        .unwrap();
    assert!(found && score == 1.0, "{score}");
    assert!(
        (x + 3.0).abs() < 0.6 && z.abs() < 0.6,
        "the ring point nearest the bot: {x}, {z}"
    );
    let all: Table = lua
        .load("return EnvironmentQuery.RunAll(bot, spec, { target = player })")
        .eval()
        .unwrap();
    assert_eq!(all.raw_len(), 8);
    let first: Table = all.get(1).unwrap();
    let last: Table = all.get(8).unwrap();
    assert!(first.get::<f32>("score").unwrap() >= last.get::<f32>("score").unwrap());
    assert_eq!(first.get::<f32>("x").unwrap(), x);
}

#[test]
fn nothing_surviving_is_false_and_empty_and_draws_nothing() {
    let (lua, rng) = vm();
    let before = rng.borrow().state();
    let code = r#"
        local spec = { generator = { points = { { 500, 0, 500 } } } }
        local f, x = EnvironmentQuery.Run(bot, spec, nil, "random_best_25")
        return f, x, #EnvironmentQuery.RunAll(bot, spec)"#;
    let got: (bool, f32, i64) = lua.load(code).eval().unwrap();
    assert_eq!(got, (false, 0.0, 0));
    assert_eq!(rng.borrow().state(), before, "no draw");
    lua.load(RING).exec().unwrap();
    lua.load("EnvironmentQuery.Run(bot, spec, { target = player }, 'random_best_25')")
        .exec()
        .unwrap();
    let mut once = Random::new(9);
    once.value();
    assert_eq!(rng.borrow().state(), once.state(), "one draw");
}

#[test]
fn mistakes_are_errors_naming_their_field() {
    let (lua, _) = vm();
    lua.load(RING).exec().unwrap();
    let cases = [
        (
            "spec.tests[2].distance.to = 'targte'",
            "tests[2].distance.to: unknown context 'targte'",
        ),
        ("spec.tests[2].scroe = 1", "tests[2]: unknown field 'scroe'"),
        (
            "spec.generator.donut.outer = nil",
            "generator.donut.outer: required",
        ),
        ("spec.generator.donut.per_ring = 600", "past the 512"),
        (
            "spec.tests[3] = { distance = {} }",
            "tests[3]: neither filters nor scores",
        ),
        (
            "spec.tests[3] = { dot = { from = 'spot' } }",
            "tests[3].dot.from: a point has no facing",
        ),
        ("spec.tests[3] = { path = 'maybe' }", "tests[3].path"),
    ];
    for (edit, want) in cases {
        lua.load(RING).exec().unwrap();
        let code = format!(
            "{edit}; EnvironmentQuery.Run(bot, spec, {{ target = player, spot = {{ 1, 0, 1 }} }})"
        );
        let got = error(&lua, &code);
        assert!(got.contains(want), "{edit}: {got}");
    }
    let mode = error(
        &lua,
        "EnvironmentQuery.Run(bot, spec, { target = player }, 'worst')",
    );
    assert!(mode.contains("unknown mode 'worst'"), "{mode}");
}

#[test]
fn a_destroyed_or_inactive_context_or_querier_is_an_error() {
    let (lua, _) = vm();
    lua.load(RING).exec().unwrap();
    let dead = error(&lua, "EnvironmentQuery.Run(bot, spec, { target = 999 })");
    assert!(
        dead.contains("contexts.target: entity 999 is destroyed or inactive"),
        "{dead}"
    );
    let none = error(&lua, "EnvironmentQuery.Run(nil, spec, { target = player })");
    assert!(
        none.contains("EnvironmentQuery.Run: no entity (nil id)"),
        "{none}"
    );
    let missing = error(&lua, "EnvironmentQuery.RunAll(bot, spec)");
    assert!(missing.contains("unknown context 'target'"), "{missing}");
}
