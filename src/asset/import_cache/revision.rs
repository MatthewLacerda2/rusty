//! src/asset/import_cache/revision.rs — which edit of a geometry a mesh holds.
//!
//! It lives with the importer because an import hands it out (#857): every
//! instance of one cached import shares its revision, so a renderer's shared mesh
//! slot is uploaded once, not once per spawn. `components::MeshRevision` is this
//! type.

use std::sync::atomic::{AtomicU64, Ordering};

/// Which edit of a mesh's geometry its vertices are (#1540). Every rebuilt or new
/// geometry takes a fresh one, newer than any before it; a clone keeps it, since
/// it holds the same vertices. Each renderer remembers the revision it uploaded per
/// geometry and re-uploads when a mesh carries a newer one, so a change reaches
/// every renderer drawing the scene, however many there are. Never saved: a loaded
/// mesh is new geometry, or the import a cached one was built from (#857). The
/// value is never read by the sim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MeshRevision(u64);

/// The next revision handed out.
static NEXT_REVISION: AtomicU64 = AtomicU64::new(1);

impl MeshRevision {
    /// A revision newer than every one handed out before.
    pub fn next() -> Self {
        Self(NEXT_REVISION.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for MeshRevision {
    /// New geometry: a fresh revision.
    fn default() -> Self {
        Self::next()
    }
}
