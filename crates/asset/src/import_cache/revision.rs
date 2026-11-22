use std::sync::atomic::{AtomicU64, Ordering};

/// Which edit of a mesh's geometry its vertices are. Every rebuilt or new geometry
/// takes a fresh, newer one; a clone keeps it. Instances of one cached import
/// share it, so a renderer uploads the mesh once. Each renderer re-uploads when it
/// sees a newer one. Never saved; never read by the sim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MeshRevision(u64);

static NEXT_REVISION: AtomicU64 = AtomicU64::new(1);

impl MeshRevision {
    pub fn next() -> Self {
        Self(NEXT_REVISION.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for MeshRevision {
    fn default() -> Self {
        Self::next()
    }
}
