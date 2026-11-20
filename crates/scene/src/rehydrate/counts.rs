//! How many meshes this thread rebuilt from their reference: what a test holds a
//! restore that keeps live geometry to. A test hook (crates/CLAUDE.md): compiled in
//! every build, since the tests that read it are `rusty-edit`'s, and counting only
//! on a thread that asked [`rebuilds`] once.

use std::cell::Cell;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static REBUILDS: Cell<usize> = const { Cell::new(0) };
}

/// Count one mesh (a Mesh, or a collider's own mesh) given geometry by a rebuild.
pub(super) fn count_rebuild() {
    if ARMED.get() {
        REBUILDS.set(REBUILDS.get() + 1);
    }
}

/// Meshes this thread rebuilt from their reference (each one a copy of its
/// geometry) since the first call of this function on it, which starts the count.
#[doc(hidden)]
pub fn rebuilds() -> usize {
    ARMED.set(true);
    REBUILDS.get()
}
