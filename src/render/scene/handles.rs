//! src/render/scene/handles.rs — interned mesh identities (#1706): each [`MeshId`]
//! the renderer meets gets a [`MeshHandle`] once, so a draw key compares and copies a
//! `u32` instead of hashing, cloning and comparing the id's string.
//!
//! Handles are the renderer's, shared by every scene it draws, and never freed: like
//! `gpu_meshes`, the set grows with the distinct geometry a session uploads. Each also
//! records the upload revision its buffers last changed at, so a retained scene can
//! find the objects a re-upload re-shaped without walking for it every frame.

use std::collections::BTreeMap;

use crate::render::MeshId;

/// One interned [`MeshId`]: an index into [`MeshHandles`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MeshHandle(pub u32);

/// Every mesh identity the renderer has met, by handle.
#[derive(Default)]
pub(crate) struct MeshHandles {
    index: BTreeMap<MeshId, MeshHandle>,
    /// Per handle: its id, and the [`Self::uploads`] count at its last (re-)upload.
    ids: Vec<(MeshId, u64)>,
    /// GPU uploads ever made under any handle.
    uploads: u64,
}

impl MeshHandles {
    /// `id`'s handle, interning it on first sight (its one clone).
    pub(crate) fn intern(&mut self, id: &MeshId) -> MeshHandle {
        if let Some(&handle) = self.index.get(id) {
            return handle;
        }
        let handle = MeshHandle(self.ids.len() as u32);
        self.ids.push((id.clone(), 0));
        self.index.insert(id.clone(), handle);
        handle
    }

    /// The id `handle` names.
    pub(crate) fn id(&self, handle: MeshHandle) -> &MeshId {
        &self.ids[handle.0 as usize].0
    }

    /// `id`'s buffers were (re-)uploaded.
    pub(crate) fn uploaded(&mut self, id: &MeshId) {
        let handle = self.intern(id);
        self.uploads += 1;
        self.ids[handle.0 as usize].1 = self.uploads;
    }

    /// The upload count now: a scene that read up to it has seen every upload.
    pub(crate) fn uploads(&self) -> u64 {
        self.uploads
    }

    /// Whether `handle`'s buffers changed after upload count `seen`.
    pub(crate) fn uploaded_since(&self, handle: MeshHandle, seen: u64) -> bool {
        self.ids[handle.0 as usize].1 > seen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_id_one_handle_and_uploads_are_seen_once() {
        let mut h = MeshHandles::default();
        let (box_, plane) = (MeshId("Box|".into()), MeshId("Plane|".into()));
        let a = h.intern(&box_);
        assert_eq!(h.intern(&box_), a);
        let b = h.intern(&plane);
        assert_ne!(a, b);
        assert_eq!(h.id(b), &plane);
        let seen = h.uploads();
        h.uploaded(&plane);
        assert!(h.uploaded_since(b, seen) && !h.uploaded_since(a, seen));
        assert!(!h.uploaded_since(b, h.uploads()));
    }
}
