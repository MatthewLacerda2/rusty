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

/// Whether a static entity's world pose or Collider may have changed since the last ask. Not
/// `static_revision`, whose readers (shadow sweeps, probe lighting) must not re-run on a move.
/// A revision and a `WorldPose` cursor together on purpose: the pose walk that bumps the revision
/// skips a subtree still unread, so a lagging cursor would let a second move go unbumped.
pub struct StaticGeometry {
    pose: Cursor<WorldPose>,
    seen: u64,
}

impl StaticGeometry {
    /// `true` too on `Rebuild` (a clear, an Undo step, another World).
    pub fn changed(&mut self, w: &World) -> bool {
        let held = w.read(&mut self.pose, |log, seen| log.skip(seen).then_some(()));
        let now = w.changes.borrow().static_geometry;
        let moved = std::mem::replace(&mut self.seen, now) != now;
        held.is_none() || moved
    }
}
