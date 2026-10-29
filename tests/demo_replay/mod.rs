//! Demos (#1581): a recorded run replays bit for bit, and a changed seed is
//! reported as a divergence at its tick.

use std::path::{Path, PathBuf};

use rusty::dev::demo::{self, Demo, Destination, Replayer, Verdict};
use rusty::dev::harness::Harness;
use rusty::dev::scenario;

mod hosts;
mod play_bin;

/// Walk, turn, a REPL line and API calls that move the world and draw from
/// `Random` between ticks: every kind of act a demo carries.
const SCENARIO: &str = r#"
local player = Scene.FindEntityByName("Player")
Input.Press("W")
Harness.Step(90)
Input.AddMouseDelta(12.5, -3.25)
Input.Release("W")
Input.Press("D")
Transform.SetPosition(player, 1.5, 0, -2)
local r = Random.Value()
Harness.Step(150)
Input.Release("D")
Harness.Step(60)
"#;

/// Run [`SCENARIO`] recording into `<dir>/run.demo`; the demo's path.
pub fn record(dir: &Path, scenario_code: &str) -> PathBuf {
    let path = dir.join("run.demo");
    let harness = Harness::new(dir.join("record"), "");
    demo::record(
        &mut harness.world.borrow_mut(),
        Destination::File(path.clone()),
    );
    let report = scenario::run_code(harness, scenario_code, "=record").unwrap();
    assert!(report.passed);
    path
}

/// Replay `demo` on a fresh default-scene harness to its end, `perturb` applied
/// after it is prepared; the verdict.
pub fn replay(dir: &Path, demo: Demo, perturb: impl FnOnce(&Harness)) -> Verdict {
    let harness = Harness::new(dir.join("replay"), "");
    let ticks = demo.ticks;
    demo::prepare(&mut harness.world.borrow_mut(), demo).unwrap();
    perturb(&harness);
    harness.step(u32::try_from(ticks).unwrap());
    let mut world = harness.world.borrow_mut();
    let replayer = world.tap_mut::<Replayer>().unwrap();
    assert!(replayer.finished());
    replayer.verdict().clone()
}

#[test]
fn a_recorded_run_replays_bit_for_bit() {
    let dir = crate::temp::dir().join("demo_bit_for_bit");
    let path = record(&dir, SCENARIO);
    let demo = Demo::read(&path).unwrap();
    assert_eq!(demo.ticks, 300, "the scenario stepped 300 ticks");
    let calls: Vec<_> = demo.ticks().flat_map(|t| t.host).collect();
    assert!(calls.len() >= 3, "the API calls are recorded: {calls:?}");
    let verdict = replay(&dir, demo, |_| {});
    assert_eq!(verdict.diverged, None, "{verdict:?}");
    assert_eq!(verdict.checked, 2, "a digest every 120 ticks");
    // The default scene is built in memory: no file to replay it from, said once.
    let taints: Vec<u64> = verdict.taints.iter().map(|(t, _)| *t).collect();
    assert_eq!(taints, [0], "{:?}", verdict.taints);
}

#[test]
fn a_changed_seed_diverges_at_the_first_digest() {
    let dir = crate::temp::dir().join("demo_seed");
    let demo = Demo::read(&record(&dir, SCENARIO)).unwrap();
    let seed = demo.header.seed;
    let verdict = replay(&dir, demo, |h| {
        let mut world = h.world.borrow_mut();
        world.resources.script_manager.set_play_seed(seed ^ 1);
    });
    let diverged = verdict.diverged.expect("a different seed is caught");
    assert_eq!((diverged.tick, diverged.matched), (119, None));
}

#[test]
fn a_replay_through_a_scenario_reports_in_its_results() {
    let dir = crate::temp::dir().join("demo_results");
    let demo = Demo::read(&record(&dir, SCENARIO)).unwrap();
    let harness = Harness::new(dir.join("replay"), "");
    let ticks = demo.ticks;
    demo::prepare(&mut harness.world.borrow_mut(), demo).unwrap();
    let report = scenario::run_code(harness, &format!("Harness.Step({ticks})"), "=demo").unwrap();
    assert!(report.passed);
    let results = std::fs::read_to_string(report.results_path).unwrap();
    assert!(
        results.contains("the demo replays bit for bit"),
        "{results}"
    );
    assert!(
        results.contains("demo: replayed 300 of 300 ticks"),
        "{results}"
    );
}
