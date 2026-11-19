//! The verb timers on a played scene: off unless armed, and an armed run plays
//! the same frames as a plain one.

use crate::harness::Harness;

use super::super::verbs::{arm, armed, take};

fn played(dir: &str) -> (String, super::super::verbs::Frame) {
    let out = rusty_test_support::temp::dir().join(dir);
    let h = Harness::new(out, "assets/scripts/bot.lua");
    take();
    h.step(90);
    let frame = take();
    (h.snapshot().to_string(), frame)
}

#[test]
fn a_plain_run_wraps_nothing() {
    let (_, frame) = played("verbs_plain");
    assert!(!armed());
    assert!(frame.verbs.is_empty(), "{:?}", frame.verbs);
}

#[test]
fn an_armed_run_times_the_bots_verbs_and_plays_the_same_frames() {
    let (plain, _) = played("verbs_before");
    arm();
    let (timed, frame) = played("verbs_after");
    assert_eq!(plain, timed, "the wrappers change nothing a frame shows");
    assert!(!frame.verbs.is_empty(), "the bot calls verbs every tick");
    assert!(frame
        .verbs
        .iter()
        .all(|v| v.name.contains('.') && v.calls > 0));
    assert!(
        frame.update_ms > 0.0,
        "update_scripts' verbs are summed apart"
    );
}
