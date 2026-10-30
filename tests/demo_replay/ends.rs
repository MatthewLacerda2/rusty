//! How a replay starts and ends: the pause state Play started with, and a replay
//! that stops before the demo does.

use rusty::dev::demo::{self, Demo, Replayer};
use rusty::dev::harness::Harness;
use rusty::dev::scenario;

use super::{record, SCENARIO};

#[test]
fn the_pause_play_started_with_is_replayed() {
    let dir = crate::temp::dir().join("demo_paused");
    let code = "Harness.Step(5)";
    let path = dir.join("run.demo");
    let h = Harness::new(dir.join("record"), "");
    demo::record(
        &mut h.world.borrow_mut(),
        demo::Destination::File(path.clone()),
    );
    {
        // `Time.Pause()` then `Time.Step(300)` in edit mode: Play keeps both.
        let world = h.world.borrow();
        let mut time = world.resources.time.borrow_mut();
        (time.paused, time.pending_steps) = (true, 300);
    }
    assert!(scenario::run_code(h, code, "=record").unwrap().passed);
    let demo = Demo::read(&path).unwrap();
    assert!(demo.header.paused && demo.header.pending_steps == 300);
    let h = Harness::new(dir.join("replay"), "");
    demo::prepare(&mut h.world.borrow_mut(), demo).unwrap();
    let world = h.world.borrow();
    let time = world.resources.time.borrow();
    assert!(time.paused && time.pending_steps == 300);
}

/// Replay the [`SCENARIO`] demo for 100 of its 300 ticks; the results.
fn stop_short(dir: &str, by_scenario: bool) -> (bool, String) {
    let dir = crate::temp::dir().join(dir);
    let demo = Demo::read(&record(&dir, SCENARIO)).unwrap();
    let harness = Harness::new(dir.join("replay"), "");
    demo::prepare(&mut harness.world.borrow_mut(), demo).unwrap();
    if by_scenario {
        let mut world = harness.world.borrow_mut();
        world.tap_mut::<Replayer>().unwrap().driven_by_scenario();
    }
    let report = scenario::run_code(harness, "Harness.Step(100)", "=demo").unwrap();
    let results = std::fs::read_to_string(report.results_path).unwrap();
    (report.passed, results)
}

#[test]
fn a_replay_that_stops_short_fails_unless_a_scenario_stopped_it() {
    let (passed, results) = stop_short("demo_short_stop", false);
    assert!(
        !passed && results.contains("stopped at tick 100 of 300"),
        "{results}"
    );
    let (passed, results) = stop_short("demo_short_scenario", true);
    assert!(
        passed && results.contains("as far as the scenario stepped it"),
        "{results}"
    );
}
