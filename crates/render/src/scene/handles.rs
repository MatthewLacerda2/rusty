//! Mesh handles are shared by every scene and never freed: `residency` frees a mesh's
//! buffers, not its handle.

use std::collections::BTreeMap;

use crate::MeshId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MeshHandle(pub u32);

#[derive(Default)]
pub(crate) struct MeshHandles {
    index: BTreeMap<MeshId, MeshHandle>,
    ids: Vec<(MeshId, u64)>,
    uploads: u64,
}

impl MeshHandles {
    pub(crate) fn intern(&mut self, id: &MeshId) -> MeshHandle {
        if let Some(&handle) = self.index.get(id) {
            return handle;
        }
        let handle = MeshHandle(self.ids.len() as u32);
        self.ids.push((id.clone(), 0));
        self.index.insert(id.clone(), handle);
        handle
    }

    pub(crate) fn id(&self, handle: MeshHandle) -> &MeshId {
        &self.ids[handle.0 as usize].0
    }

    pub(crate) fn uploaded(&mut self, id: &MeshId) {
        let handle = self.intern(id);
        self.uploads += 1;
        self.ids[handle.0 as usize].1 = self.uploads;
    }

    pub(crate) fn uploads(&self) -> u64 {
        self.uploads
    }

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
