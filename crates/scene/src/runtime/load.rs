use std::collections::BTreeSet;

use crate::Scene;

impl Scene {
    pub fn request_load(&mut self, path: String) {
        self.loads.request(path);
    }

    /// The mark lasts until the entity is destroyed or Play stops.
    pub fn dont_destroy_on_load(&mut self, id: u32) -> bool {
        let exists = self.world.contains(id);
        if exists {
            self.loaded.persistent.insert(id);
        }
        exists
    }

    pub fn load_survivors(&self) -> BTreeSet<u32> {
        self.loaded
            .persistent
            .iter()
            .flat_map(|&id| self.world.self_and_descendants(id))
            .collect()
    }
}
