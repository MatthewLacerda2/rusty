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

mod control;
mod step_pump;

use rusty::dev::harness::Harness;
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
