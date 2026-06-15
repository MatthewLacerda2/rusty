//! src/app/system.rs — System abstraction
//!
//! A system is per-frame logic: a plain function the schedule calls once per tick,
//! in registration order, within its stage. No trait ceremony, no event bus —
//! modules self-register their systems via `register(&mut App)` (see `app::App`).
//!
//! The conceptual model (CLAUDE.md) is `fn(&mut World, &mut Resources)`. Until the
//! Rc<RefCell> graph is replaced with &mut World/Resources threading (issue #39),
//! the world *and* its resources both live behind `GameWorld`, so a system here is
//! `fn(&mut GameWorld, f32)`: the world it mutates plus the frame's scaled `dt`.
//! When #39 lands, this alias becomes the canonical two-argument form and the
//! call sites move with it — the schedule plumbing above does not change.
//!
//! Allowed deps: app::*.

use super::game::GameWorld;

/// One unit of per-frame logic, run by the schedule in registration order.
///
/// `dt` is the scaled per-frame delta (`Time::delta_time`), the same value the
/// hand-wired play loop passed to every system before the schedule existed.
pub type System = fn(&mut GameWorld, f32);
