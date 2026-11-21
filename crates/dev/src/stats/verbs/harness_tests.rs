use crate::harness::Harness;

use crate::stats::verbs::{arm, armed, take, Frame};

fn played(dir: &str) -> (String, Frame) {
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
    let armed_run = std::thread::spawn(|| {
        arm();
        played("verbs_after")
    });
    let (timed, frame) = armed_run.join().expect("the armed run");
    let (_, after) = played("verbs_plain_again");
    assert!(
        !armed() && after.verbs.is_empty(),
        "the arm stayed on its thread"
    );
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
