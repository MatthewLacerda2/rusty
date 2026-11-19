//! crates/physics/src/character/cost.rs — how much query work a `Move` does, counted
//! on this thread: what the cost test (`cost_tests`) holds a Move to, so a
//! change that visits more triangles or runs more queries fails it on any
//! machine, without a clock. A test hook: compiled in, and counting only where
//! `debug_assertions` holds (every test profile) on a thread whose test armed
//! it with [`take`]. A release or ship build compiles [`count`] to nothing, so
//! the path it measures pays nothing for it; a debug build that never arms it
//! pays one thread-local read per count.

use std::cell::Cell;

/// What one count records.
#[derive(Clone, Copy)]
pub(super) enum Work {
    /// One run of rapier's `move_shape` (each holds two contact-manifold
    /// passes and two or more shape casts).
    Run,
    /// One shape cast against a triangle mesh or compound, part by part.
    MeshCast,
    /// One part (a triangle) a mesh cast visited.
    Part,
    /// One part's normal constraints built (`triangle_normal_constraints`).
    Constraints,
    /// One GJK `contact` query an exact normal took.
    Contact,
}

/// The counts since the last [`take`], one per [`Work`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Counts {
    pub(super) runs: u64,
    pub(super) mesh_casts: u64,
    pub(super) parts: u64,
    pub(super) constraints: u64,
    pub(super) contacts: u64,
}

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static COUNTS: Cell<Counts> = const { Cell::new(Counts {
        runs: 0,
        mesh_casts: 0,
        parts: 0,
        constraints: 0,
        contacts: 0,
    }) };
}

/// One more `work`, when armed; nothing otherwise, and nothing at all without
/// `debug_assertions`.
#[inline(always)]
pub(super) fn count(work: Work) {
    if cfg!(debug_assertions) && ARMED.with(Cell::get) {
        COUNTS.with(|c| {
            let mut n = c.get();
            let slot = match work {
                Work::Run => &mut n.runs,
                Work::MeshCast => &mut n.mesh_casts,
                Work::Part => &mut n.parts,
                Work::Constraints => &mut n.constraints,
                Work::Contact => &mut n.contacts,
            };
            *slot += 1;
            c.set(n);
        });
    }
}

/// The counts since the last call on this thread, which arms counting here and
/// starts them again from zero.
///
/// # Panics
/// In a build without `debug_assertions`, where nothing is counted: run the
/// test in a debug or test profile.
#[cfg(test)]
pub(super) fn take() -> Counts {
    if !cfg!(debug_assertions) {
        panic!("character::cost counts only with debug_assertions on; run this test in a debug or test profile");
    }
    ARMED.with(|a| a.set(true));
    COUNTS.with(|c| c.replace(Counts::default()))
}
