//! src/render/scene/table.rs — the renderer's object table (#1706): one slot per kept
//! object, holding the [`ObjectData`] the forward pass reads through a camera's
//! instances, mirrored on the GPU and written there only where it changed.
//!
//! Every kept scene allocates its objects' slots from the one table, so a preview tab
//! and the main scene never share a slot and a switch between them uploads nothing.
//! A slot is freed when its object goes (a despawn, a mesh removed, a rebuild, the
//! scene evicted) and reused by the next object. Slot 0 is [`ObjectData::IDENTITY`]
//! for good: overlays name it. Mesh particles, which are no kept object, get
//! *transient* slots past the kept ones for one camera's draws
//! ([`ObjectTable::push_transient`]), written with that camera's upload.
//!
//! The GPU side is `gpu::draw_buffers::DrawBuffers::sync_objects`: it writes the
//! dirty slots in runs, or the whole table when the buffer had to grow.

use std::ops::Range;

use crate::render::gpu::uniforms::ObjectData;

pub(crate) struct ObjectTable {
    /// Every slot's record, freed ones included (unread until reused).
    records: Vec<ObjectData>,
    free: Vec<u32>,
    /// Slots written since the last [`Self::take_dirty`], in any order.
    dirty: Vec<u32>,
    /// One camera's mesh particles, at slots `records.len()..`.
    transient: Vec<ObjectData>,
}

impl Default for ObjectTable {
    fn default() -> Self {
        Self {
            records: vec![ObjectData::IDENTITY],
            free: Vec::new(),
            // The identity reaches the GPU with the first upload.
            dirty: vec![0],
            transient: Vec::new(),
        }
    }
}

impl ObjectTable {
    /// A slot for a new object, reusing a freed one first. A new slot is marked
    /// dirty: what the GPU holds there (zeros, or a past camera's transient record)
    /// is not what the table holds, even when its first write leaves it unchanged.
    pub(crate) fn alloc(&mut self) -> u32 {
        debug_assert!(
            self.transient.is_empty(),
            "no slot moves under a camera's particles"
        );
        self.free.pop().unwrap_or_else(|| {
            let slot = self.records.len() as u32;
            self.records.push(ObjectData::IDENTITY);
            self.dirty.push(slot);
            slot
        })
    }

    /// `slot`'s object is gone: the slot is reused by a later [`Self::alloc`].
    pub(crate) fn release(&mut self, slot: u32) {
        debug_assert!(
            slot != 0 && !self.free.contains(&slot),
            "slot {slot} freed twice"
        );
        self.free.push(slot);
    }

    /// Make `slot` hold `data`; marked for upload only when it changed.
    pub(crate) fn write(&mut self, slot: u32, data: ObjectData) {
        let at = &mut self.records[slot as usize];
        if *at != data {
            *at = data;
            self.dirty.push(slot);
        }
    }

    /// What `slot` holds.
    pub(crate) fn get(&self, slot: u32) -> &ObjectData {
        &self.records[slot as usize]
    }

    /// A transient slot holding `data` for the current camera's draws (a mesh
    /// particle), dropped by [`Self::end_camera`].
    pub(crate) fn push_transient(&mut self, data: ObjectData) -> u32 {
        self.transient.push(data);
        (self.records.len() + self.transient.len() - 1) as u32
    }

    /// The camera's draws were uploaded: its transient slots are gone.
    pub(crate) fn end_camera(&mut self) {
        self.transient.clear();
    }

    /// Every kept slot's record, freed ones included, then the transient ones: the
    /// whole array as the GPU holds it after a sync.
    pub(crate) fn records(&self) -> (&[ObjectData], &[ObjectData]) {
        (&self.records, &self.transient)
    }

    /// The runs of slots written since the last call, each once, ascending.
    pub(crate) fn take_dirty(&mut self) -> Vec<Range<u32>> {
        self.dirty.sort_unstable();
        self.dirty.dedup();
        let mut runs: Vec<Range<u32>> = Vec::new();
        for slot in self.dirty.drain(..) {
            match runs.last_mut() {
                Some(run) if run.end == slot => run.end += 1,
                _ => runs.push(slot..slot + 1),
            }
        }
        runs
    }

    /// Slots in use, the identity included: what the kept scenes hold.
    #[cfg(test)]
    pub(crate) fn live(&self) -> usize {
        self.records.len() - self.free.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32) -> ObjectData {
        let mut data = ObjectData::IDENTITY;
        data.model_matrix[12] = x;
        data
    }

    #[test]
    fn slots_are_reused_and_only_changes_are_dirty() {
        let mut t = ObjectTable::default();
        assert_eq!(t.take_dirty(), vec![0..1], "the identity is uploaded first");
        let (a, b, c) = (t.alloc(), t.alloc(), t.alloc());
        assert_eq!((a, b, c), (1, 2, 3), "slot 0 is the identity's");
        // A new slot is uploaded even when its record is the identity: the GPU
        // holds zeros there.
        t.write(b, ObjectData::IDENTITY);
        assert_eq!(t.take_dirty(), vec![1..4]);
        t.write(a, at(1.0));
        t.write(c, at(3.0));
        t.write(b, ObjectData::IDENTITY); // unchanged: not dirty
        assert_eq!(t.take_dirty(), vec![1..2, 3..4]);
        t.write(b, at(2.0));
        t.write(a, at(1.5));
        t.write(a, at(1.5));
        assert_eq!(t.take_dirty(), vec![1..3], "adjacent slots join, each once");
        assert!(t.take_dirty().is_empty());
        t.release(b);
        assert_eq!(t.live(), 3);
        assert_eq!(t.alloc(), b, "a freed slot is reused");
        assert!(
            t.take_dirty().is_empty(),
            "the GPU already holds what it held"
        );
        assert_eq!(t.get(c).model_matrix[12], 3.0);
    }

    #[test]
    fn transient_slots_follow_the_kept_ones_for_one_camera() {
        let mut t = ObjectTable::default();
        t.alloc();
        assert_eq!(t.push_transient(at(7.0)), 2);
        assert_eq!(t.push_transient(at(8.0)), 3);
        assert_eq!(t.records().1.len(), 2);
        t.end_camera();
        assert!(t.records().1.is_empty());
        t.take_dirty();
        assert_eq!(t.alloc(), 2, "a kept slot after the camera");
        assert_eq!(t.take_dirty(), vec![2..3], "over the particle's record");
    }
}
