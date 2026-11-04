//! The display scale (#1715) is a sim input a demo records, as it records the Game
//! view's size: a replay reads the recording's, never the monitor it plays on.

use rusty::dev::demo::{self, Demo, Destination};
use rusty::dev::harness::Harness;
use rusty::dev::scenario;

#[test]
fn a_replay_reads_the_display_scale_the_demo_recorded() {
    let dir = crate::temp::dir().join("demo_display_scale");
    let path = dir.join("run.demo");
    let harness = Harness::new(dir.join("record"), "");
    harness
        .world
        .borrow()
        .resources
        .screen
        .borrow_mut()
        .set_display_scale(2.0);
    demo::record(
        &mut harness.world.borrow_mut(),
        Destination::File(path.clone()),
    );
    let report = scenario::run_code(harness, "Harness.Step(3)", "=record").unwrap();
    assert!(report.passed);
    let demo = Demo::read(&path).unwrap();
    assert_eq!(demo.header.display_scale, Some(2.0));

    let replay = Harness::new(dir.join("replay"), "");
    replay
        .world
        .borrow()
        .resources
        .screen
        .borrow_mut()
        .set_display_scale(3.0);
    demo::prepare(&mut replay.world.borrow_mut(), demo).unwrap();
    replay.step(1);
    let world = replay.world.borrow();
    assert_eq!(world.resources.screen.borrow().display_scale(), 2.0);
    let read = (world.resources.script_manager).eval("Video.GetDisplayScale()");
    assert_eq!(read.unwrap().parse::<f32>().ok(), Some(2.0));
}
