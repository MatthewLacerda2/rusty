//! crates/ecs/src/moves.rs — how many times a column was attached to or detached
//! from a live entity on this thread: each one moves the entity to another hecs
//! archetype, copying every column it has. What lets a test hold that a per-tick
//! system edits a component where it sits instead of taking it out and putting it
//! back. Counted by the column funnel (`World::set_component` attaching a column the
//! entity lacked, `World::take_component` detaching one it held); a write over a
//! column the entity already holds moves nothing and is not counted. A test hook:
//! compiled in every build, since the test that reads it is another crate's, and
//! counting only on a thread that asked [`count`] once, so a build that never asks
//! pays one thread-local read per move. Whole-entity placement (spawn, despawn,
//! restore) is not a column move and is not counted.

use std::cell::Cell;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static MOVES: Cell<u64> = const { Cell::new(0) };
}

pub(crate) fn note() {
    if ARMED.with(Cell::get) {
        MOVES.with(|m| m.set(m.get() + 1));
    }
}

/// Archetype moves (a column attached to or detached from a live entity) on this
/// thread since the first call of this function on it, which starts the count.
pub fn count() -> u64 {
    ARMED.with(|a| a.set(true));
    MOVES.with(Cell::get)
}
