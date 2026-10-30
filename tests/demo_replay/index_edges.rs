//! Demos (#1597), the edges: what no index line may cost, a seek after the game
//! quit or ran past the end, a restart whose demo is gone, a cut on another demo.

use std::path::Path;

use rusty::dev::demo::{self, Demo, Replayer};
use rusty::dev::scenario;

use super::index::{digest_at, replaying};
use super::{record, replay, SCENARIO};

/// The follow camera goes where the player does: nowhere.
const NAN_PLAYER: &str = r#"
Transform.SetPosition(Scene.FindEntityByName("Player"), 0/0, 0, 0)
Harness.Step(30)
"#;

#[test]
fn a_nan_camera_or_a_broken_index_line_never_costs_the_replay() {
    let dir = crate::temp::dir().join("demo_index_nan");
    let path = record(&dir, NAN_PLAYER);
    let text = std::fs::read_to_string(&path).unwrap();
    let (header, rest) = text.split_once('\n').unwrap();
    assert!(!rest.contains("null"), "no NaN written as null: {rest}");
    let broken = format!("{header}\n{{\"at\":3,\"camera\":[null,0.0,0.0]}}\n{rest}");
    std::fs::write(&path, broken).unwrap();
    let demo = Demo::read(&path).expect("a bad index line is skipped");
    assert_eq!(demo.index().filter(|l| l.camera.is_some()).count(), 0);
    let verdict = replay(&dir, demo, |_| {});
    assert_eq!((verdict.ticks, verdict.diverged), (30, None));
}

#[test]
fn a_seek_back_after_the_game_quit_starts_unquit() {
    let dir = crate::temp::dir().join("demo_index_quit");
    let path = record(&dir, "Harness.Step(100)\nApplication.Quit()\n");
    let (_harness, world) = replaying(&dir, "quit", &path);
    demo::seek(&mut world.borrow_mut(), 99).unwrap();
    assert!(world.borrow().quit_requested(), "the last tick quits");
    demo::seek(&mut world.borrow_mut(), 50).expect("restarted, unquit");
    assert!(!world.borrow().quit_requested());
}

#[test]
fn a_seek_after_stepping_past_the_end_restarts() {
    let dir = crate::temp::dir().join("demo_index_past_end");
    let path = record(&dir, SCENARIO);
    let (_straight, world) = replaying(&dir, "straight", &path);
    let at_299 = digest_at(&world, 299);
    let (harness, world) = replaying(&dir, "past", &path);
    harness.step(310);
    assert_eq!(
        digest_at(&world, 299),
        at_299,
        "back where tick 299 left it"
    );
}

#[test]
fn a_restart_whose_demo_is_gone_leaves_the_run_as_it_stood() {
    let dir = crate::temp::dir().join("demo_index_gone");
    let path = record(&dir, SCENARIO);
    let (_harness, world) = replaying(&dir, "gone", &path);
    demo::seek(&mut world.borrow_mut(), 200).unwrap();
    let moved = dir.join("moved.demo");
    std::fs::rename(&path, &moved).unwrap();
    assert!(demo::seek(&mut world.borrow_mut(), 60).is_err());
    let ticks = |world: &mut rusty::app::GameWorld| {
        let replayer = world.tap_mut::<Replayer>().expect("the replay stays");
        replayer.verdict().ticks
    };
    assert_eq!(ticks(&mut world.borrow_mut()), 201);
    std::fs::rename(&moved, &path).unwrap();
    demo::seek(&mut world.borrow_mut(), 60).unwrap();
    assert_eq!(ticks(&mut world.borrow_mut()), 61);
}

#[test]
fn a_cut_refuses_to_run_on_another_demo() {
    let root = crate::temp::dir().join("demo_index_wrong");
    let (ours, theirs) = (root.join("ours"), root.join("theirs"));
    let (path, other) = (record(&ours, SCENARIO), record(&theirs, SCENARIO));
    let cut = root.join("cut.lua");
    let full = demo::full_path(&path).to_string_lossy().into_owned();
    std::fs::write(&cut, demo::cut_scenario(&path, 0, 99)).unwrap();
    let text = std::fs::read_to_string(&cut).unwrap();
    assert!(text.contains(&full) && !text.contains("SeekDemo"), "{text}");
    let run = |on: &Path| {
        let (harness, _) = replaying(&root, "run", on);
        scenario::run_file(harness, &cut).is_ok_and(|report| report.passed)
    };
    assert!(run(&path), "on its own demo");
    assert!(!run(&other), "on another");
}
