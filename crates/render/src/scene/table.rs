//! crates/render/src/scene/table.rs — the renderer's slot tables (#1706): one slot per kept
//! record, mirrored on the GPU and written there only where it changed. The object
//! table ([`ObjectTable`]) holds one [`ObjectData`] per kept object, which the forward
//! pass reads through a camera's instances; the material table (`records.rs`) holds
//! one `MaterialData` per kept (scene, library material, variant), which a draw's
//! uniform names.
//!
//! Every kept scene allocates its objects' slots from the one table, so a preview tab
//! and the main scene never share a slot and a switch between them uploads nothing.
//! A slot is freed when its object goes (a despawn, a mesh removed, the scene
//! evicted) and reused by the next object; a rebuild keeps every surviving
//! object's slot, so it uploads only the records whose value moved. Slot 0 is [`ObjectData::IDENTITY`]
//! for good: overlays name it. Mesh particles, which are no kept object, get
//! *transient* slots past the kept ones for one camera's draws
//! ([`ObjectTable::push_transient`]), written with that camera's upload.
//!
//! The GPU side is `gpu::draw_buffers::DrawBuffers::sync_objects`: it writes the
//! dirty slots in runs, or the whole table when the buffer had to grow.

use std::ops::Range;

use crate::gpu::uniforms::ObjectData;

/// How many clean slots a dirty run may span to join the next one. Measured on
/// lavapipe in release (#1706): one `write_buffer` call costs about 1.2 µs over its
/// bytes, about what 8 KB of copy costs, so a gap of 32 records (7.5 KB) costs about what
/// a split would. Each call also allocates in wgpu (about four allocations).
pub(crate) const JOIN_GAP: u32 = 32;

/// A record a [`SlotTable`] holds: slot 0 holds [`Slotted::identity`] for good.
pub(crate) trait Slotted: Copy + PartialEq + bytemuck::Pod {
    fn identity() -> Self;
}

impl Slotted for ObjectData {
    fn identity() -> Self {
        ObjectData::IDENTITY
    }
}

/// The object table: every kept object's record, by slot.
pub(crate) type ObjectTable = SlotTable<ObjectData>;

pub(crate) struct SlotTable<T> {
    /// Every slot's record, freed ones included (unread until reused).
    records: Vec<T>,
    free: Vec<u32>,
    /// Slots written since the last [`Self::take_dirty`], in any order.
    dirty: Vec<u32>,
    /// One camera's mesh particles, at slots `records.len()..`.
    transient: Vec<T>,
    /// Which slots are free, by slot: what checks a double release in linear time
    /// (a scene's release frees every slot it held).
    #[cfg(debug_assertions)]
    freed: Vec<bool>,
}

impl<T: Slotted> Default for SlotTable<T> {
    fn default() -> Self {
        Self {
            records: vec![T::identity()],
            free: Vec::new(),
            // The identity reaches the GPU with the first upload.
            dirty: vec![0],
            transient: Vec::new(),
            #[cfg(debug_assertions)]
            freed: vec![false],
        }
    }
}

impl<T: Slotted> SlotTable<T> {
    /// A slot for a new object, reusing a freed one first. A new slot is marked
    /// dirty: what the GPU holds there (zeros, or a past camera's transient record)
    /// is not what the table holds, even when its first write leaves it unchanged.
    pub(crate) fn alloc(&mut self) -> u32 {
        debug_assert!(
            self.transient.is_empty(),
            "no slot moves under a camera's particles"
        );
        let slot = self.free.pop().unwrap_or_else(|| {
            let slot = self.records.len() as u32;
            self.records.push(T::identity());
            self.dirty.push(slot);
            slot
        });
        #[cfg(debug_assertions)]
        {
            self.freed.resize(self.records.len(), false);
            self.freed[slot as usize] = false;
        }
        slot
    }

    /// `slot`'s object is gone: the slot is reused by a later [`Self::alloc`].
    pub(crate) fn release(&mut self, slot: u32) {
        #[cfg(debug_assertions)]
        {
            let freed = &mut self.freed[slot as usize];
            assert!(slot != 0 && !*freed, "slot {slot} freed twice");
            *freed = true;
        }
        self.free.push(slot);
    }

    /// Make `slot` hold `data`; marked for upload only when it changed.
    pub(crate) fn write(&mut self, slot: u32, data: T) {
        let at = &mut self.records[slot as usize];
        if *at != data {
            *at = data;
            self.dirty.push(slot);
        }
    }

    /// What `slot` holds.
    pub(crate) fn get(&self, slot: u32) -> &T {
        &self.records[slot as usize]
    }

    /// A transient slot holding `data` for the current camera's draws (a mesh
    /// particle), dropped by [`Self::end_camera`].
    pub(crate) fn push_transient(&mut self, data: T) -> u32 {
        self.transient.push(data);
        (self.records.len() + self.transient.len() - 1) as u32
    }

    /// The camera's draws were uploaded: its transient slots are gone.
    pub(crate) fn end_camera(&mut self) {
        self.transient.clear();
    }

    /// Every kept slot's record, freed ones included, then the transient ones: the
    /// whole array as the GPU holds it after a sync.
    pub(crate) fn records(&self) -> (&[T], &[T]) {
        (&self.records, &self.transient)
    }

    /// The runs of slots written since the last call, each once, ascending. Runs
    /// fewer than [`JOIN_GAP`] slots apart are joined, the slots between rewritten
    /// with what they already hold: each run is one `queue.write_buffer`, and so
    /// one staging allocation, so scattered movers cost a few writes, not one each.
    pub(crate) fn take_dirty(&mut self) -> Vec<Range<u32>> {
        self.dirty.sort_unstable();
        self.dirty.dedup();
        let mut runs: Vec<Range<u32>> = Vec::new();
        for slot in self.dirty.drain(..) {
            match runs.last_mut() {
                Some(run) if slot - run.end < JOIN_GAP => run.end = slot + 1,
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
        assert_eq!(
            t.take_dirty(),
            vec![1..4],
            "a short gap is rewritten, not split"
        );
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
    fn runs_join_across_a_short_gap_only() {
        let mut t = ObjectTable::default();
        (0..3 * JOIN_GAP).for_each(|_| _ = t.alloc());
        t.take_dirty();
        // JOIN_GAP - 1 clean slots between 2 and `near` join; JOIN_GAP after it do not.
        let (near, far) = (2 + JOIN_GAP, 3 + 2 * JOIN_GAP);
        for slot in [1, 2, near, far] {
            t.write(slot, at(slot as f32));
        }
        assert_eq!(t.take_dirty(), vec![1..near + 1, far..far + 1]);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "freed twice")]
    fn a_slot_freed_twice_is_caught() {
        let mut t = ObjectTable::default();
        let a = t.alloc();
        t.release(a);
        t.release(a);
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
