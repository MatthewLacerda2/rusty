//! `EnvironmentQuery.Request` / `Poll` / `Cancel` through Lua (#2116): pending
//! until the queue serves it, then exactly `Run`'s answer once, then gone.

use mlua::Lua;

use super::tests::{error, vm, RING};
use crate::api::ApiCtx;

/// Serve the VM's query queue one tick, as the play loop's query phase does.
fn serve(lua: &Lua) {
    let c = ApiCtx::default();
    let (queue, nav, rng) = (
        c.queries.get(lua).unwrap(),
        c.nav.get(lua).unwrap(),
        c.random.get(lua).unwrap(),
    );
    queue
        .borrow_mut()
        .serve(&nav.borrow(), &mut rng.borrow_mut());
}

type Answer = (
    String,
    Option<bool>,
    Option<f32>,
    Option<f32>,
    Option<f32>,
    Option<f32>,
);

fn poll(lua: &Lua) -> Answer {
    lua.load("return EnvironmentQuery.Poll(ticket)")
        .eval()
        .unwrap()
}

#[test]
fn a_request_is_pending_until_served_then_answers_what_run_does_once() {
    let (lua, _) = vm();
    lua.load(RING).exec().unwrap();
    let run: (bool, f32, f32, f32, f32) = lua
        .load("return EnvironmentQuery.Run(bot, spec, { target = player })")
        .eval()
        .unwrap();
    lua.load("ticket = EnvironmentQuery.Request(bot, spec, { target = player })")
        .exec()
        .unwrap();
    assert_eq!(poll(&lua).0, "pending", "nothing ran yet");
    serve(&lua);
    let (state, found, x, y, z, score) = poll(&lua);
    assert_eq!(state, "done");
    assert_eq!(
        (found, x, y, z, score),
        (
            Some(run.0),
            Some(run.1),
            Some(run.2),
            Some(run.3),
            Some(run.4)
        )
    );
    assert_eq!(poll(&lua).0, "gone", "an answer is read once");
}

#[test]
fn cancel_drops_a_request_and_a_stranger_ticket_is_gone() {
    let (lua, _) = vm();
    lua.load(RING).exec().unwrap();
    let (first, again, state, stranger): (bool, bool, String, String) = lua
        .load(
            "ticket = EnvironmentQuery.Request(bot, spec, { target = player }, 'random_best_5')
             return EnvironmentQuery.Cancel(ticket), EnvironmentQuery.Cancel(ticket),
                    EnvironmentQuery.Poll(ticket), EnvironmentQuery.Poll(-3)",
        )
        .eval()
        .unwrap();
    assert!(first && !again);
    assert_eq!((state.as_str(), stranger.as_str()), ("gone", "gone"));
}

#[test]
fn the_budget_is_set_and_read_and_never_below_one() {
    let (lua, _) = vm();
    let (default, set, floor): (u32, u32, u32) = lua
        .load(
            "local d = EnvironmentQuery.GetIterationsPerFrame()
             EnvironmentQuery.SetIterationsPerFrame(300)
             local s = EnvironmentQuery.GetIterationsPerFrame()
             EnvironmentQuery.SetIterationsPerFrame(-5)
             return d, s, EnvironmentQuery.GetIterationsPerFrame()",
        )
        .eval()
        .unwrap();
    assert_eq!(
        (default, set, floor),
        (crate::ai::query::DEFAULT_ITERATIONS_PER_FRAME, 300, 1)
    );
}

#[test]
fn a_request_with_a_bad_mode_or_querier_is_a_lua_error() {
    let (lua, _) = vm();
    lua.load(RING).exec().unwrap();
    let bad = error(
        &lua,
        "EnvironmentQuery.Request(bot, spec, { target = player }, 'worst')",
    );
    assert!(
        bad.contains("EnvironmentQuery.Request: unknown mode 'worst'"),
        "{bad}"
    );
    let none = error(&lua, "EnvironmentQuery.Request(nil, spec)");
    assert!(
        none.contains("EnvironmentQuery.Request: no entity"),
        "{none}"
    );
}
