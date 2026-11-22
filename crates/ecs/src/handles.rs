//! Stable id to hecs handle, a dense `Vec` rather than a hash map: ids are monotonic from 1, so a
//! lookup is one bounds check. A dead id keeps its 8-byte slot until a `clear`.

#[derive(Default)]
pub(super) struct Handles(Vec<Option<hecs::Entity>>);

impl Handles {
    #[inline]
    pub fn get(&self, id: u32) -> Option<hecs::Entity> {
        self.0.get(id as usize).copied().flatten()
    }

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

    pub fn clear(&mut self) {
        self.0 = Vec::new();
    }
}

#[cfg(test)]
#[path = "handles_tests.rs"]
mod tests;
