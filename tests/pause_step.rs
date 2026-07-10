//! Windowed pause / fixed-dt step / resume control (issue #283).
//!
//! The windowed frame loop itself (`main.rs`) needs a window + GPU and can't be
//! instantiated in a test, so we test the two things that *make* it correct:
//!
//! 1. The pause/step **control state machine** on the `Time` resource, end-to-end
//!    through the `Time` Lua bindings (the very surface the in-app console and the
//!    external command channel reach) — `Time.Pause()` / `Time.Step(n)` / `Time.Resume()`.
//! 2. The **step pump** the windowed loop runs: while paused, drain
//!    `take_pending_step` one `FIXED_DELTA_TIME` tick at a time. We mirror that exact
//!    loop here against a real `GameWorld` and assert (a) it advances by exactly N
//!    fixed frames and (b) the same N steps from the same state are deterministic —
//!    the harness determinism contract, now on the windowed step path.
//!
//! Gated on `dev` (needs the harness + the command channel's session).
#![cfg(feature = "dev")]

use rusty::dev::harness::{Harness, FIXED_DT};
use rusty::dev::session::Session;
use rusty::time::FIXED_DELTA_TIME;

/// The windowed loop's paused step pump, mirrored over a `GameWorld`: while paused,
/// drain every queued step at the fixed dt (exactly what `main::advance_sim` does on
/// a paused frame). Returns how many fixed ticks ran.
fn drain_steps(h: &Harness) -> u32 {
    let mut ran = 0;
    loop {
        let mut world = h.world.borrow_mut();
        let take = world.time().borrow_mut().take_pending_step();
        if !take {
            break;
        }
        world.tick(FIXED_DELTA_TIME);
        ran += 1;
    }
    ran
}

/// Read an entity's world position. The `let` binding is load-bearing: it drops the
/// transform guard before `scene` goes out of scope, so the borrow checker is
/// satisfied (returning the field expression directly would outlive `scene`).
#[allow(clippy::let_and_return)]
fn entity_pos(h: &Harness, id: u32) -> glam::Vec3 {
    let world = h.world.borrow();
    let scene = world.scene().borrow();
    let pos = scene.world.transform(id).unwrap().position;
    pos
}

/// Resolve the demo Player entity id.
fn player_id(h: &Harness) -> u32 {
    let world = h.world.borrow();
    let scene = world.scene().borrow();
    scene.find_entity_by_name("Player").expect("Player exists")
}

/// The harness `FIXED_DT` and the engine `FIXED_DELTA_TIME` must be the same value —
/// windowed stepping reuses the harness step semantics, so they cannot drift.
#[test]
fn windowed_step_uses_the_harness_fixed_dt() {
    assert_eq!(
        FIXED_DELTA_TIME, FIXED_DT,
        "windowed step pump must reuse the harness fixed dt"
    );
}

#[test]
fn step_pump_advances_exactly_n_fixed_frames_then_halts() {
    let h = Harness::new(std::env::temp_dir().join("rusty_pause_step_a"), "");
    let start = h.frame();

    // Pause, queue 5 steps, drain them: exactly 5 fixed ticks run, then the pump halts.
    h.world.borrow().time().borrow_mut().pause();
    h.world.borrow().time().borrow_mut().request_steps(5);
    let ran = drain_steps(&h);

    assert_eq!(
        ran, 5,
        "the pump must run exactly the queued number of steps"
    );
    assert_eq!(
        h.frame(),
        start + 5,
        "world advanced by exactly 5 fixed frames"
    );
    assert_eq!(
        h.world.borrow().time().borrow().pending_steps,
        0,
        "no steps left after the pump drains"
    );
    // A second drain with nothing queued is a no-op (halted).
    assert_eq!(drain_steps(&h), 0);
    assert_eq!(h.frame(), start + 5);
}

#[test]
fn the_same_n_steps_from_the_same_state_are_deterministic() {
    // N fixed steps from state S must yield an identical S′ — the determinism the
    // windowed step path inherits from the fixed-dt harness semantics.
    let run = || {
        let h = Harness::new(std::env::temp_dir().join("rusty_pause_step_det"), "");
        // Drive a known motion so the snapshot is non-trivial, then step deterministically.
        h.world
            .borrow()
            .input()
            .borrow_mut()
            .set_key_state("W", true);
        h.world.borrow().time().borrow_mut().pause();
        h.world.borrow().time().borrow_mut().request_steps(120);
        drain_steps(&h);
        h.snapshot().to_string()
    };
    assert_eq!(
        run(),
        run(),
        "same steps from same state must match exactly"
    );
}

#[test]
fn paused_world_does_not_move_until_stepped() {
    // A paused world with no queued steps is frozen: the pump runs nothing and the
    // frame count does not advance (rendering would still run in the real loop).
    let h = Harness::new(std::env::temp_dir().join("rusty_pause_step_frozen"), "");
    h.world
        .borrow()
        .input()
        .borrow_mut()
        .set_key_state("W", true);

    let player = player_id(&h);
    let pos_before = entity_pos(&h, player);

    h.world.borrow().time().borrow_mut().pause();
    assert_eq!(
        drain_steps(&h),
        0,
        "a paused, step-less frame advances nothing"
    );

    let pos_after = entity_pos(&h, player);
    assert_eq!(pos_before, pos_after, "the frozen world must not move");
}

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
    let h = Harness::new(std::env::temp_dir().join("rusty_pause_not_stop"), "");
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
