//! src/app/system.rs — System abstraction
//!
//! A system is per-frame logic: a plain function the schedule calls once per tick,
//! in registration order, within its stage. No trait ceremony, no event bus —
//! modules self-register their systems via `register(&mut App)` (see `app::App`).
//!
//! The conceptual model (CLAUDE.md) is `fn(&mut World, &mut Resources)`. This is now
//! the canonical form (issue #39): the world of record is `app::World` (a handle to
//! the hecs-backed `Scene`, #40) threaded as `&mut World`, and the engine singletons
//! ride alongside in `&mut Resources`. The borrow checker keeps the world and the
//! resources distinct at the system boundary — there is no longer one opaque
//! `GameWorld` blob the systems reach through. The scaled per-frame `dt` lives on
//! `Resources` (`Resources::dt`), so systems take no third argument.
//!
//! Allowed deps: app::*.

use super::resources::Resources;
use super::world::World;

/// One unit of per-frame logic, run by the schedule in registration order.
///
/// Takes the world of record (`&mut World`) and the engine singletons
/// (`&mut Resources`). The frame's scaled delta is `Resources::dt` — the same value
/// the hand-wired play loop passed to every system before the schedule existed.
pub type System = fn(&mut World, &mut Resources);
