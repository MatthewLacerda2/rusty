//! The `Time` pause/step/resume verbs, and pause keeping play state (unlike Stop).

use super::{drain_steps, entity_pos, player_id};
use rusty::dev::harness::Harness;
use rusty::dev::session::Session;

// --- API round-trip: the verbs reached identically by the console + command channel ---

#[test]
fn time_verbs_round_trip_through_the_command_surface() {
    // The session is the headless half of the one evaluator the windowed command
    // channel and in-app console also use, so this proves the verbs are reachable
    // identically from every caller.
    let s = Session::new("").expect("session boots");

    // Pause: the flag flips, observable through the same surface.
    s.eval("Time.Pause()").unwrap();
    assert_eq!(s.eval("Time.IsPaused()").unwrap(), "true");

    // Step(3) while paused enqueues exactly 3.
    s.eval("Time.Step(3)").unwrap();
    assert_eq!(s.eval("Time.PendingSteps()").unwrap(), "3");

    // Resume clears both paused and pending — back to real-time play.
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

// --- Pause must NOT restore the edit snapshot (contrast with Stop) ---

#[test]
fn pause_keeps_play_state_unlike_stop() {
    // Mutate the world in play mode, then pause: the mutation must survive (pause is a
    // freeze, not a reset). This is the exact behaviour Stop does NOT have — Stop runs
    // `exit_play` and restores the edit snapshot, discarding the mutation.
    let h = Harness::new(crate::temp::dir().join("rusty_pause_not_stop"), "");
    let player = player_id(&h);
    // Step once so play mode is fully entered, then mutate.
    h.step(1);
    {
        let world = h.world.borrow();
        let mut scene = world.scene().borrow_mut();
        scene.world.transform_mut(player).unwrap().position.x = 42.0;
    }

    // Pause + a step does not reset: the mutated position persists.
    h.world.borrow().time().borrow_mut().pause();
    h.world.borrow().time().borrow_mut().request_steps(1);
    drain_steps(&h);

    let x_after_pause = entity_pos(&h, player).x;
    assert!(
        (x_after_pause - 42.0).abs() < 1.0,
        "pause must keep play-mode state (mutation survived): x={x_after_pause}"
    );
    assert!(
        h.world.borrow().is_playing(),
        "pause does not leave play mode (that's Stop)"
    );
}
