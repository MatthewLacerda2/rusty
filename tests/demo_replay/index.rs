//! Demos (#1597): the index finds a moment, a seek lands on it bit for bit either
//! way, and a cut scenario reproduces its range.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use rusty::app::GameWorld;
use rusty::dev::demo::{self, Demo, EventKind, Query, Replayer};
use rusty::dev::harness::Harness;
use rusty::dev::scenario;

use super::record;

/// A mark with a value, a console line, an enemy destroyed.
const MARKED: &str = r#"
Input.Press("W")
Harness.Step(100)
Debug.Mark("checkpoint", { hp = 90 })
Debug.Log("reached the gate")
Harness.Step(50)
Scene.DestroyEntity(Scene.FindEntityByName("Enemy_1"))
Harness.Step(150)
"#;

/// A replay of `path` on a fresh harness, stepped by a scenario, its world kept
/// to read after a run.
pub(super) fn replaying(dir: &Path, name: &str, path: &Path) -> (Harness, Rc<RefCell<GameWorld>>) {
    let harness = Harness::new(dir.join(name), "");
    let mut world = harness.world.borrow_mut();
    demo::prepare(&mut world, Demo::read(path).unwrap()).unwrap();
    // As `play --demo <file> <scenario>`: the scenario may stop it short.
    world.tap_mut::<Replayer>().unwrap().driven_by_scenario();
    drop(world);
    let world = Rc::clone(&harness.world);
    (harness, world)
}

pub(super) fn digest_at(world: &Rc<RefCell<GameWorld>>, tick: u64) -> String {
    demo::seek(&mut world.borrow_mut(), tick).unwrap();
    demo::state_digest(&world.borrow())
}

#[test]
fn the_index_finds_a_mark_a_log_line_and_a_destroyed_entity() {
    let dir = crate::temp::dir().join("demo_index_find");
    let demo = Demo::read(&record(&dir, MARKED)).unwrap();
    assert_eq!(demo.find(&Query::Mark("checkpoint".into())), [100]);
    assert_eq!(demo.find(&Query::Log("the gate".into())), [100]);
    assert_eq!(demo.find(&Query::Event(EventKind::Destroyed)), [150]);
    assert_eq!(
        demo.find(&Query::Event(EventKind::Play)),
        [0, 299],
        "Stop at the last tick"
    );
    assert!(
        demo.find(&Query::Mark("check".into())).is_empty(),
        "a label whole"
    );
    let samples = demo.index().filter(|line| line.camera.is_some()).count();
    assert_eq!(samples, 20, "the camera four times a second");
    // The same, asked from a scenario on the demo.
    let (harness, _) = replaying(&dir, "find", demo.path());
    let code = r#"
        local found = Harness.FindInDemo({ mark = "checkpoint" })
        Harness.Expect(#found == 1 and found[1] == 100, "the mark at 100")
        local gone = Harness.FindInDemo({ event = "destroyed" })
        Harness.Expect(#gone == 1 and gone[1] == 150, "the enemy at 150")
        local all = Harness.FindInDemo({ inside = { min = {-1e9, -1e9, -1e9}, max = {1e9, 1e9, 1e9} } })
        Harness.Expect(#all == 20, "every camera sample")
        Harness.SeekDemo(150)
        Harness.Expect(Scene.FindEntityByName("Enemy_1") == nil, "gone at 150")
    "#;
    let report = scenario::run_code(harness, code, "=find").unwrap();
    let results = std::fs::read_to_string(report.results_path).unwrap();
    assert!(report.passed, "{results}");
}

#[test]
fn a_seek_either_way_lands_on_the_straight_replays_state() {
    let dir = crate::temp::dir().join("demo_index_seek");
    let path = record(&dir, MARKED);
    let (_straight, world) = replaying(&dir, "straight", &path);
    let (at_60, at_200) = (digest_at(&world, 60), digest_at(&world, 200));
    let (_seeking, world) = replaying(&dir, "seeking", &path);
    assert_eq!(digest_at(&world, 200), at_200, "forward");
    assert_eq!(digest_at(&world, 60), at_60, "back: restarted");
    assert_eq!(digest_at(&world, 200), at_200, "forward again");
    let mut world = world.borrow_mut();
    let verdict = world.tap_mut::<Replayer>().unwrap().verdict().clone();
    assert_eq!((verdict.ticks, verdict.diverged), (201, None));
    assert!(demo::seek(&mut world, 300).is_err(), "past the end");
}

#[test]
fn a_cut_scenario_reproduces_its_range() {
    let dir = crate::temp::dir().join("demo_index_cut");
    let path = record(&dir, MARKED);
    let cut = dir.join("cut.lua");
    let (harness, _) = replaying(&dir, "cutting", &path);
    let code = format!("Harness.CutDemo(130, 250, {:?})", cut.to_string_lossy());
    assert!(scenario::run_code(harness, &code, "=cut").unwrap().passed);
    let text = std::fs::read_to_string(&cut).unwrap();
    assert!(text.contains("Harness.SeekDemo(129)") && text.contains("Harness.Step(121)"));
    let (_straight, world) = replaying(&dir, "straight", &path);
    let at_250 = digest_at(&world, 250);
    let (harness, world) = replaying(&dir, "run_cut", &path);
    let report = scenario::run_file(harness, &cut).unwrap();
    let results = std::fs::read_to_string(report.results_path).unwrap();
    assert!(report.passed, "{results}");
    assert!(
        results.contains("as far as the scenario stepped it (tick 251 of 300)"),
        "{results}"
    );
    assert_eq!(demo::state_digest(&world.borrow()), at_250);
}
