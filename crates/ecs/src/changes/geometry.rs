use super::{Cursor, WorldPose};
use crate::world::World;

impl World {
    pub fn static_geometry(&self) -> StaticGeometry {
        StaticGeometry {
            pose: self.change_cursor(),
            seen: self.changes.borrow().static_geometry,
        }
    }
}

pub struct StaticGeometry {
    pose: Cursor<WorldPose>,
    seen: u64,
}

impl StaticGeometry {
    pub fn changed(&mut self, w: &World) -> bool {
        let held = w.read(&mut self.pose, |log, seen| log.skip(seen).then_some(()));
        let now = w.changes.borrow().static_geometry;
        let moved = std::mem::replace(&mut self.seen, now) != now;
        held.is_none() || moved
    }
}
