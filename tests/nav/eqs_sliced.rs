//! The time-sliced `EnvironmentQuery` in the play loop (#2116): a query requested
//! in one `Update` is answered by a later one, with exactly what `Run` answered in
//! that first `Update`; a fresh Play starts with the default budget, and two runs
//! answer the same on the same tick.

use std::cell::RefCell;
use std::rc::Rc;

use rusty::app::GameWorld;
use rusty::core::input::InputState;
use rusty::navigation::NavigationGraph;
use rusty::scripting::ConsoleLogs;

use super::off_floor::default_scene_with;

/// Asks on its first `Update` with a small budget, then polls: prints the budget
/// it found, then the tick the answer came and whether it matches `Run`'s.
const ASKER: &str = r#"
local Q = {}
local spec = {
    generator = { donut = { context = "target", inner = 4, outer = 10, rings = 2, per_ring = 12 } },
    tests = { { path = "exists" }, { path = "length", score = { curve = "inverse" } } },
}
function Q.Start(id)
    print("budget " .. EnvironmentQuery.GetIterationsPerFrame())
    EnvironmentQuery.SetIterationsPerFrame(256)
    Q.n = 0
end
function Q.Update(id, dt)
    Q.n = Q.n + 1
    if Q.n == 1 then
        local ctx = { target = { 6, 0, -6 } }
        Q.want = { EnvironmentQuery.Run(id, spec, ctx) }
        Q.ticket = EnvironmentQuery.Request(id, spec, ctx)
    elseif Q.ticket ~= nil then
        local state, found, x, y, z, score = EnvironmentQuery.Poll(Q.ticket)
        if state ~= "pending" then
            local w = Q.want
            local same = state == "done" and found == w[1] and x == w[2] and y == w[3]
                and z == w[4] and score == w[5]
            print(string.format("answered %d %s %s", Q.n, tostring(found), tostring(same)))
            Q.ticket = nil
        end
    end
end
return Q
"#;

/// Play the default scene with Enemy_1 running [`ASKER`] for `ticks`, twice over
/// (Stop between): what it printed.
fn play_twice(ticks: usize) -> Vec<String> {
    let script = crate::temp::file("eqs_asker.lua");
    std::fs::write(&script, ASKER).unwrap();
    let (scene, _) = default_scene_with(script.to_str().unwrap());
    let nav = NavigationGraph::from_scene(&scene);
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let mut world = GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::clone(&console),
    );
    let mark = console.borrow().logged();
    for _ in 0..2 {
        world.set_playing(true);
        crate::play::run(&mut world, ticks);
        world.set_playing(false);
        world.tick(crate::play::DT);
    }
    let logs = console.borrow();
    let printed = logs.since(mark).map(|(m, _)| m.clone());
    printed
        .filter(|m| m.starts_with("budget") || m.starts_with("answered"))
        .collect()
}

#[test]
fn a_request_answers_runs_pick_ticks_later_the_same_in_every_play() {
    let lines = play_twice(240);
    let default = rusty::ai::query::DEFAULT_ITERATIONS_PER_FRAME;
    let [budget, answer, again, answer_again] = &lines[..] else {
        panic!("a budget and an answer per Play: {lines:?}");
    };
    assert_eq!(budget, &format!("budget {default}"));
    assert_eq!(again, budget, "Stop puts the budget back");
    assert!(
        answer.ends_with(" true true"),
        "found, and Run's pick: {answer}"
    );
    let tick: u32 = answer.split(' ').nth(1).unwrap().parse().unwrap();
    assert!(tick > 3, "256 expansions a tick slice the search: {answer}");
    assert_eq!(answer_again, answer, "the same answer on the same tick");
}
