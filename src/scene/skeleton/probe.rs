//! How many times a skeleton's bone list was read, in tests only (#1617): the
//! proof that a system skips skeletons it has no business with (a scene without
//! a ragdoll costs `pose_ragdolls` no bone work). A no-op outside `cfg(test)`.

#[cfg(test)]
thread_local! {
    static READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Count one read of a skeleton's bone list.
pub(crate) fn read() {
    #[cfg(test)]
    READS.with(|n| n.set(n.get() + 1));
}

/// The reads this thread has counted so far.
#[cfg(test)]
pub(crate) fn reads() -> usize {
    READS.with(|n| n.get())
}
