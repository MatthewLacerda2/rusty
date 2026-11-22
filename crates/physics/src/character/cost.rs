use std::cell::Cell;

#[derive(Clone, Copy)]
pub(super) enum Work {
    Run,
    MeshCast,
    Part,
    Constraints,
    Contact,
}

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
