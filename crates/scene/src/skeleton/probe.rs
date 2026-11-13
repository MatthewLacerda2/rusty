//! How many times a skeleton's bone list was read (#1617): the proof that a system
//! skips skeletons it has no business with (a scene without a ragdoll costs
//! `pose_ragdolls` no bone work). A test hook: compiled in every build, since the
//! test that reads it is another crate's, and counting only on a thread that asked
//! [`reads`] once, so a build that never asks pays one thread-local read per read.

use std::cell::Cell;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static READS: Cell<usize> = const { Cell::new(0) };
}

/// Count one read of a skeleton's bone list, on a thread that armed the count.
pub(crate) fn read() {
    if ARMED.with(Cell::get) {
        READS.with(|n| n.set(n.get() + 1));
    }
}

/// The reads counted on this thread since its first call of this function, which
/// starts the count.
pub fn reads() -> usize {
    ARMED.with(|a| a.set(true));
    READS.with(Cell::get)
}
