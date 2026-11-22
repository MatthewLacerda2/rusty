use std::ops::Range;

use crate::gpu::uniforms::ObjectData;

/// Clean slots a dirty run may span to join the next: 32 records (8 KB) cost about one
/// `write_buffer` call (lavapipe, release).
pub(crate) const JOIN_GAP: u32 = 32;

pub(crate) trait Slotted: Copy + PartialEq + bytemuck::Pod {
    fn identity() -> Self;
}

impl Slotted for ObjectData {
    fn identity() -> Self {
        ObjectData::IDENTITY
    }
}

pub(crate) type ObjectTable = SlotTable<ObjectData>;

pub(crate) struct SlotTable<T> {
    records: Vec<T>,
    free: Vec<u32>,
    /// Slots written since the last [`Self::take_dirty`], in any order.
    dirty: Vec<u32>,
    transient: Vec<T>,
    #[cfg(debug_assertions)]
    freed: Vec<bool>,
}

impl<T: Slotted> Default for SlotTable<T> {
    fn default() -> Self {
        Self {
            records: vec![T::identity()],
            free: Vec::new(),
            dirty: vec![0],
            transient: Vec::new(),
            #[cfg(debug_assertions)]
            freed: vec![false],
        }
    }
}

impl<T: Slotted> SlotTable<T> {
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

    pub(crate) fn release(&mut self, slot: u32) {
        #[cfg(debug_assertions)]
        {
            let freed = &mut self.freed[slot as usize];
            assert!(slot != 0 && !*freed, "slot {slot} freed twice");
            *freed = true;
        }
        self.free.push(slot);
    }

    pub(crate) fn write(&mut self, slot: u32, data: T) {
        let at = &mut self.records[slot as usize];
        if *at != data {
            *at = data;
            self.dirty.push(slot);
        }
    }

    pub(crate) fn get(&self, slot: u32) -> &T {
        &self.records[slot as usize]
    }

    pub(crate) fn push_transient(&mut self, data: T) -> u32 {
        self.transient.push(data);
        (self.records.len() + self.transient.len() - 1) as u32
    }

    pub(crate) fn end_camera(&mut self) {
        self.transient.clear();
    }

    pub(crate) fn records(&self) -> (&[T], &[T]) {
        (&self.records, &self.transient)
    }

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
        t.write(b, ObjectData::IDENTITY);
        assert_eq!(t.take_dirty(), vec![1..4]);
        t.write(a, at(1.0));
        t.write(c, at(3.0));
        t.write(b, ObjectData::IDENTITY);
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
