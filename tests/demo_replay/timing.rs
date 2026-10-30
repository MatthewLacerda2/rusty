//! When things happen in a demo: each tick's `dt`, Play entered from a paused
//! frame, and the digest Play ends on.

use rusty::dev::demo::{self, Demo, Destination};
use rusty::dev::harness::{Harness, FIXED_DT};

use super::{record, replay};

/// A harness recording into `<dir>/run.demo`, and that path.
fn recording(dir: &std::path::Path) -> (Harness, std::path::PathBuf) {
    let path = dir.join("run.demo");
    let h = Harness::new(dir.join("record"), "");
    demo::record(&mut h.world.borrow_mut(), Destination::File(path.clone()));
    (h, path)
}

#[test]
fn a_wall_clock_dt_is_recorded_and_replayed() {
    let dir = crate::temp::dir().join("demo_dt");
    let (mut h, path) = recording(&dir);
    for i in 0..250u16 {
        // An editor's frame times: never quite the fixed step.
        let dt = 0.0125 + f32::from(i % 7) * 0.001;
        h.world.borrow_mut().tick(dt);
    }
    demo::conclude(&mut h);
    drop(h);

    let demo = Demo::read(&path).unwrap();
    assert!(
        demo.ticks().any(|t| t.dt != FIXED_DT),
        "the dt is in the demo"
    );
    // The replay steps the fixed dt; the tap hands each tick its recorded one.
    let verdict = replay(&dir, demo, |_| {});
    assert_eq!(
        (&verdict.diverged, verdict.checked),
        (&None, 3),
        "{verdict:?}"
    );
}

#[test]
fn play_entered_from_a_paused_frame_replays() {
    let dir = crate::temp::dir().join("demo_paused_entry");
    let (mut h, path) = recording(&dir);
    // A paused editor: Play is entered by the frame's poll, no tick runs.
    assert!(h.world.borrow_mut().poll_transition() == rusty::app::PlayTransition::Entered);
    // A line typed before the first Step: entering Play would have reset the draw.
    h.world
        .borrow()
        .script_manager()
        .eval("Random.Value()")
        .unwrap();
    h.step(20);
    demo::conclude(&mut h);
    drop(h);

    let demo = Demo::read(&path).unwrap();
    let first = demo.ticks().next().unwrap();
    assert_eq!(
        first.host.len(),
        1,
        "the line is tick 0's: {:?}",
        first.host
    );
    let verdict = replay(&dir, demo, |_| {});
    assert_eq!(
        (&verdict.diverged, verdict.checked),
        (&None, 1),
        "{verdict:?}"
    );
}

#[test]
fn a_demo_shorter_than_a_digest_is_checked_at_its_end() {
    let dir = crate::temp::dir().join("demo_short");
    let code = "Harness.Step(5)\nRandom.Value()\nHarness.Step(5)";
    let demo = Demo::read(&record(&dir, code)).unwrap();
    assert!(demo.end.digest.is_some(), "Stop's digest");
    let seed = demo.header.seed;
    let verdict = replay(&dir, demo, |h| {
        let mut world = h.world.borrow_mut();
        world.resources.script_manager.set_play_seed(seed ^ 1);
    });
    let diverged = verdict
        .diverged
        .expect("a different seed is caught at the end");
    assert_eq!((diverged.tick, verdict.checked), (10, 1));
}
