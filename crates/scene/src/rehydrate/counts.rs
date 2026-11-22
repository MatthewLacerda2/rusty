//! A test hook (`crates/CLAUDE.md`): the meshes this thread rebuilt from their reference,
//! counted once [`rebuilds`] was called on it. `rusty-edit`'s tests hold a restore to it.

use std::cell::Cell;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static REBUILDS: Cell<usize> = const { Cell::new(0) };
}

pub(super) fn count_rebuild() {
    if ARMED.get() {
        REBUILDS.set(REBUILDS.get() + 1);
    }
}

#[doc(hidden)]
pub fn rebuilds() -> usize {
    ARMED.set(true);
    REBUILDS.get()
}
