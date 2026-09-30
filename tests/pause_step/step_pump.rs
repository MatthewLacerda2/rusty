//! The paused step pump: exact fixed-dt stepping, determinism, and a frozen world.

use super::{drain_steps, entity_pos, player_id};
use rusty::dev::harness::{Harness, FIXED_DT};
use rusty::time::FIXED_DELTA_TIME;

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
