use crate::Scene;

impl Scene {
    pub fn update_all_colliders(&mut self) {
        self.world.flush_bounds();
        let ids = self.world.ids().to_vec();
        for id in ids {
            self.world.refresh_collider_aabb(id);
        }
    }
}

#[cfg(test)]
mod guard_tests;
#[cfg(test)]
mod tests;
