//! Host acts in a demo: a REPL line replays, an act it cannot carry taints it.

use rusty::dev::demo::{self, Demo, Destination};
use rusty::dev::harness::Harness;
use rusty::scripting::HostCall;

use super::{record, replay};

#[test]
fn a_repl_line_between_ticks_replays() {
    let dir = crate::temp::dir().join("demo_repl");
    let path = dir.join("run.demo");
    let h = Harness::new(dir.join("record"), "");
    demo::record(&mut h.world.borrow_mut(), Destination::File(path.clone()));
    h.step(10);
    // The console's and MCP's one evaluator, between two ticks.
    let line = "Random.Value(); Transform.SetPosition(Scene.FindEntityByName('Player'), 4, 0, 4)";
    h.world.borrow().script_manager().eval(line).unwrap();
    h.step(230);
    drop(h);

    let demo = Demo::read(&path).unwrap();
    let evals: Vec<_> = demo
        .ticks()
        .flat_map(|t| t.host)
        .filter(|c| matches!(c, HostCall::Eval(_)))
        .collect();
    assert_eq!(evals.len(), 1, "{evals:?}");
    let verdict = replay(&dir, demo, |_| {});
    assert_eq!((verdict.diverged, verdict.checked), (None, 2));
}

#[test]
fn an_act_a_demo_cannot_carry_taints_it_from_its_tick() {
    let dir = crate::temp::dir().join("demo_taint");
    let code = r#"
        Harness.Step(5)
        pcall(Scene.FindEntityByName, print)
        Harness.Step(5)
    "#;
    let demo = Demo::read(&record(&dir, code)).unwrap();
    let verdict = replay(&dir, demo, |_| {});
    assert_eq!(verdict.taints.len(), 1, "{verdict:?}");
    assert_eq!(verdict.taints[0].0, 5, "from the tick after the act");
}
