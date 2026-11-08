//! Graph-state callbacks through the real tick (#1901): binding enters the
//! entry state, a parameter-driven crossfade enters its target at once and
//! exits its source when it ends, a state's first marker follows its enter,
//! `GetCurrentTag` reads the active tag, and every run gives the same log.

use std::cell::RefCell;
use std::rc::Rc;

use rusty::asset::animation_graph::{self, AnimationGraph};
use rusty::scene::ScriptComponent;
use rusty::scripting::ConsoleLogs;

use super::rig::armed_scene;

const GRAPH: &str = r#"{
  "parameters": { "aim": { "Bool": false } },
  "nodes": [
    { "name": "Idle", "clip": "Slide", "is_loop": true, "tag": "calm" },
    { "name": "Aim", "clip": "AimClip", "is_loop": true, "tag": "attack",
      "events": [ { "time": 0.0, "name": "Raise" } ] } ],
  "edges": [ { "from": "Idle", "to": "Aim", "transition_duration": 0.05,
    "conditions": [ { "Bool": { "parameter": "aim", "value": true } } ] } ],
  "entry": "Idle"
}"#;

const SCRIPT: &str = r#"
local tick, updates = 0, {}
return {
  Update = function(id)
    tick = tick + 1
    if tick == 3 then Animator.SetBool(id, "aim", true) end
    if tick == 12 then print("[tag] " .. tostring(Animator.GetCurrentTag(id))) end
  end,
  OnStateEnter = function(id, info)
    print("[enter] " .. info.state .. " " .. tick .. " " .. info.layer_name .. " " .. tostring(info.tag))
  end,
  OnStateExit = function(id, info) print("[exit] " .. info.state .. " " .. tick) end,
  OnStateUpdate = function(id, info)
    updates[info.state] = (updates[info.state] or 0) + 1
    if tick == 10 then print("[updates] Idle " .. updates.Idle .. " Aim " .. updates.Aim) end
  end,
  OnAnimationEvent = function(id, name) print("[event] " .. name .. " " .. tick) end,
}"#;

/// The `[..]` lines a character playing the two-state graph prints over
/// `ticks` steps; `tag` keeps concurrent tests' temp files apart.
fn state_log(tag: &str, ticks: usize) -> Vec<String> {
    let (mut scene, hero, _, _) = armed_scene(None);
    let dir = crate::temp::dir();
    let (graph_path, script_path) = (
        dir.join(format!("rusty_1901_{tag}.animgraph")),
        dir.join(format!("rusty_1901_{tag}.lua")),
    );
    let graph: AnimationGraph = serde_json::from_str(GRAPH).unwrap();
    animation_graph::save(&graph_path, &graph).unwrap();
    std::fs::write(&script_path, SCRIPT).unwrap();
    let mut mesh = scene.world.mesh_mut(hero).unwrap();
    let mut aim = mesh.clips[0].clone();
    aim.name = "AimClip".to_string();
    mesh.clips.push(aim);
    drop(mesh);
    let slashed = |p: std::path::PathBuf| p.to_string_lossy().replace('\\', "/");
    scene.world.animator_mut(hero).unwrap().graph = Some(slashed(graph_path));
    *scene.world.scripts_mut(hero).unwrap() = vec![ScriptComponent {
        path: slashed(script_path),
        ..Default::default()
    }];
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let mut game = crate::play::world(Rc::new(RefCell::new(scene)), 5.0, Rc::clone(&console));
    game.set_playing(true);
    crate::play::run(&mut game, ticks);
    let log = console.borrow();
    let lines = log.messages.iter().map(|(m, _)| m.clone());
    lines.filter(|m| m.starts_with('[')).collect()
}

#[test]
fn states_enter_update_and_exit_on_unitys_timing() {
    let log = state_log("timing", 14);
    // A 0.05 s crossfade is three 1/60 s steps: Idle exits on the third.
    assert_eq!(
        log,
        [
            "[enter] Idle 1 Base Layer calm",
            "[enter] Aim 3 Base Layer attack",
            "[event] Raise 3",
            "[exit] Idle 5",
            "[updates] Idle 3 Aim 7",
            "[tag] attack",
        ],
        "{log:?}"
    );
}

#[test]
fn state_callbacks_are_identical_across_runs() {
    assert_eq!(state_log("run_a", 30), state_log("run_b", 30));
}
