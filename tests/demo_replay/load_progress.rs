//! A `Scene.LoadAsync`'s progress is an input (#2123): what a script read while the
//! worker and the GPU warm-up ran is recorded where it moved, and a replay reads the
//! same values on the same ticks with a loader running at another pace.

use rusty::dev::demo::{self, Demo, Destination, Replayer};
use rusty::dev::harness::Harness;

use super::load_async::{eval, level, Late};

const TICKS: usize = 12;

/// Step `TICKS` ticks, reading the operation's progress after each.
fn read(h: &Harness) -> Vec<String> {
    (0..TICKS)
        .map(|_| {
            h.step(1);
            eval(h, "return Scene.GetLoadProgress(op)")
        })
        .collect()
}

#[test]
fn a_replay_reads_the_recorded_progress_with_its_loader_at_another_pace() {
    let dir = crate::temp::dir().join("demo_load_progress");
    let level = level(&dir);
    let path = dir.join("run.demo");
    let mut h = Harness::new(dir.join("record"), "");
    demo::record(&mut h.world.borrow_mut(), Destination::File(path.clone()));
    h.world
        .borrow_mut()
        .set_scene_loader(Box::new(Late::new(9)));
    h.step(2);
    let call = format!("op = Scene.LoadAsync('{level}'); Scene.SetAllowSceneActivation(op, false)");
    eval(&h, &call);
    let recorded = read(&h);
    demo::conclude(&mut h);
    drop(h);
    assert_eq!(recorded[0], "0.09", "a tenth of the way, × 0.9");
    assert_eq!(recorded[TICKS - 1], "0.9", "ready, held: {recorded:?}");

    let demo = Demo::read(&path).unwrap();
    let moves = demo.ticks().filter(|t| t.load_progress.is_some()).count();
    assert_eq!(moves, 9, "one entry per move before ready");
    let h = Harness::new(dir.join("replay"), "");
    demo::prepare(&mut h.world.borrow_mut(), demo).unwrap();
    h.world
        .borrow_mut()
        .set_scene_loader(Box::new(Late::new(0)));
    h.step(3); // the ticks before the call, and the call's own
    let replayed: Vec<String> = std::iter::once(eval(&h, "return Scene.GetLoadProgress(op)"))
        .chain(read(&h).into_iter().take(TICKS - 1))
        .collect();
    assert_eq!(replayed, recorded);
    let mut world = h.world.borrow_mut();
    assert_eq!(
        world.tap_mut::<Replayer>().unwrap().verdict().diverged,
        None
    );
}
