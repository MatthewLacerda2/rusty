use super::{drain_steps, entity_pos, player_id};
use rusty::dev::harness::Harness;
use rusty::dev::session::Session;

#[test]
fn time_verbs_round_trip_through_the_command_surface() {
    let s = Session::new("").expect("session boots");

    s.eval("Time.Pause()").unwrap();
    assert_eq!(s.eval("Time.IsPaused()").unwrap(), "true");

    s.eval("Time.Step(3)").unwrap();
    assert_eq!(s.eval("Time.PendingSteps()").unwrap(), "3");

    s.eval("Time.Resume()").unwrap();
    assert_eq!(s.eval("Time.IsPaused()").unwrap(), "false");
    assert_eq!(s.eval("Time.PendingSteps()").unwrap(), "0");
}

#[test]
fn step_is_a_no_op_unless_paused() {
    // Stepping is only meaningful while frozen; an un-paused Step enqueues nothing,
    // so a stray Step can never silently fast-forward live play.
    let s = Session::new("").expect("session boots");
    s.eval("Time.Step(10)").unwrap();
    assert_eq!(s.eval("Time.PendingSteps()").unwrap(), "0");
}

#[test]
fn pause_keeps_play_state_unlike_stop() {
    let h = Harness::new(crate::temp::dir().join("rusty_pause_not_stop"), "");
    let player = player_id(&h);
    h.step(1);
    {
        let world = h.world.borrow();
        let mut scene = world.scene().borrow_mut();
        scene.world.transform_mut(player).unwrap().position.x = 42.0;
    }

    h.world.borrow().time().borrow_mut().pause();
    h.world.borrow().time().borrow_mut().request_steps(1);
    drain_steps(&h);

    let x_after_pause = entity_pos(&h, player).x;
    assert!(
        (x_after_pause - 42.0).abs() < 1.0,
        "pause must keep play-mode state (mutation survived): x={x_after_pause}"
    );
    assert!(
        h.world.borrow().scene().borrow().is_playing(),
        "pause does not leave play mode (that's Stop)"
    );
}

#[test]
fn harness_snapshot_reports_the_state_not_the_request() {
    // The harness asks for Play at boot, but the World enters it on the first step:
    // until then its snapshot says `editor`, as `Debug.Snapshot` does.
    let h = Harness::new(crate::temp::dir().join("rusty_snapshot_state"), "");
    assert_eq!(h.snapshot()["play_state"].as_str(), Some("editor"));
    h.step(1);
    assert_eq!(h.snapshot()["play_state"].as_str(), Some("playing"));
}
