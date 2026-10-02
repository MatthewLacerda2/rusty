//! src/ecs/handles.rs — stable engine id → hecs handle, as a dense table (#696).
//!
//! Every by-id read (`World::transform(id)`, `parent_id(id)`, …) starts here, and
//! since bones became GameObjects (#453) the per-frame skeleton paths make
//! thousands of them. Ids come from `World`'s monotonic allocator (or a saved
//! scene that came from it) and start at 1, so they are compact: a `Vec` indexed
//! by id is one bounds check and one load, where the `HashMap` it replaced paid a
//! SipHash per lookup.
//!
//! The cost is memory proportional to the highest id since the last `clear`
//! (8 bytes each, `Option<hecs::Entity>` packs into a niche), not to the live
//! count: a despawned id's slot stays allocated, as ids are never reused. Even a
//! session spawning ten entities a frame for an hour stays under 20 MB.
//!
//! The table is only ever read by id, never iterated (insertion order lives in
//! `World::order`), so it has no bearing on determinism.

#[derive(Default)]
pub(super) struct Handles(Vec<Option<hecs::Entity>>);

impl Handles {
    #[inline]
    pub fn get(&self, id: u32) -> Option<hecs::Entity> {
        self.0.get(id as usize).copied().flatten()
    }

    /// Bind (or rebind) `id`, growing the table to reach it.
    pub fn insert(&mut self, id: u32, handle: hecs::Entity) {
        let slot = id as usize;
        if slot >= self.0.len() {
            self.0.resize(slot + 1, None);
        }
        self.0[slot] = Some(handle);
    }

    pub fn remove(&mut self, id: u32) -> Option<hecs::Entity> {
        self.0.get_mut(id as usize).and_then(Option::take)
    }

    /// Unbind everything and release the table (a scene load starts over at 1).
    pub fn clear(&mut self) {
        self.0 = Vec::new();
    }
}

#[cfg(test)]
#[path = "handles_tests.rs"]
mod tests;
