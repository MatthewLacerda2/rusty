//! Animation events through the real tick (#459): a graph node's markers call the
//! character's `OnAnimationEvent` once per loop, before that tick's `LateUpdate`,
//! and identically on every run.

use std::cell::RefCell;
use std::rc::Rc;

use rusty::app::GameWorld;
use rusty::asset::animation_graph::{self, AnimationGraph};
use rusty::core::input::InputState;
use rusty::navigation::NavigationGraph;
use rusty::scene::ScriptComponent;
use rusty::scripting::ConsoleLogs;

use super::rig::{armed_scene, DT};

const GRAPH: &str = r#"{
  "nodes": [ { "name": "Slide", "clip": "Slide", "is_loop": true, "events": [
    { "time": 0.75, "name": "Fire" }, { "time": 0.25, "name": "Step" } ] } ],
  "entry": "Slide"
}"#;

const SCRIPT: &str = r#"
local tick, pending = 0, nil
return {
  Update = function(id, dt) tick = tick + 1 end,
  OnAnimationEvent = function(id, name)
    print("[ev] " .. name .. " " .. tick)
    pending = name
  end,
  LateUpdate = function(id, dt)
    if pending then print("[late] " .. pending) end
    pending = nil
  end,
}"#;

/// The `[ev]`/`[late]` lines a character playing the marked graph prints over
/// `ticks` fixed steps. `tag` keeps each caller's temp files apart (tests run
/// concurrently).
fn event_log(tag: &str, ticks: usize) -> Vec<String> {
    let (mut scene, hero, _, _) = armed_scene(None);
    let dir = crate::temp::dir();
    let (graph_path, script_path) = (
        dir.join(format!("rusty_459_{tag}.animgraph")),
        dir.join(format!("rusty_459_{tag}.lua")),
    );
    let graph: AnimationGraph = serde_json::from_str(GRAPH).unwrap();
    animation_graph::save(&graph_path, &graph).unwrap();
    std::fs::write(&script_path, SCRIPT).unwrap();
    let slashed = |p: std::path::PathBuf| p.to_string_lossy().replace('\\', "/");
    scene.world.animator_mut(hero).unwrap().graph = Some(slashed(graph_path));
    *scene.world.scripts_mut(hero).unwrap() = vec![ScriptComponent {
        path: slashed(script_path),
        ..Default::default()
    }];
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let mut game = GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -5.0, 5.0, -5.0, 5.0, 1.0,
        ))),
        Rc::clone(&console),
    );
    game.set_playing(true);
    for _ in 0..ticks {
        game.tick(DT);
    }
    let log = console.borrow();
    let lines = log.messages.iter().map(|(m, _)| m.clone());
    lines.filter(|m| m.starts_with('[')).collect()
}

#[test]
fn markers_fire_once_per_loop_in_time_order_before_late_update() {
    let log = event_log("order", 90); // 1.5 s of a 1 s loop: 0.25, 0.75, 1.25
    let names: Vec<&str> = log.iter().map(|l| l.split(' ').nth(1).unwrap()).collect();
    assert_eq!(
        names,
        ["Step", "Step", "Fire", "Fire", "Step", "Step"],
        "{log:?}"
    );
    for pair in log.chunks(2) {
        assert!(
            pair[0].starts_with("[ev]") && pair[1].starts_with("[late]"),
            "{log:?}"
        );
    }
}

#[test]
fn animation_events_are_identical_across_runs() {
    let (a, b) = (event_log("run_a", 100), event_log("run_b", 100));
    assert!(a.len() >= 6, "{a:?}");
    assert_eq!(a, b);
}
