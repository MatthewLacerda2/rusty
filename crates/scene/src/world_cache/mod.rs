use crate::Scene;

impl Scene {
    /// Asked for by hand after a load, which re-points parents wholesale; a transform or
    /// collider write refreshes its own boxes.
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
