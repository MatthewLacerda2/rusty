//! crates/ecs/src/changes/geometry.rs — [`StaticGeometry`], the watch on every static
//! entity's world pose and Collider: a revision and a `WorldPose` cursor read together.

use super::{Cursor, WorldPose};
use crate::world::World;

impl World {
    /// Watch the static geometry: every static entity's world pose and Collider.
    /// [`StaticGeometry::changed`] answers yes or no without reading ids.
    pub fn static_geometry(&self) -> StaticGeometry {
        StaticGeometry {
            pose: self.change_cursor(),
            seen: self.changes.borrow().static_geometry,
        }
    }
}

/// Whether a static entity's world pose or Collider may have changed since the last
/// ask ([`World::static_geometry`]), so a cache of static geometry skips its diff
/// without reading ids. Not [`World::static_revision`], whose readers (the shadow
/// sweeps, probe lighting) must not re-run on a static entity's move.
///
/// It is a revision and a `WorldPose` cursor held together on purpose. A pose moves
/// the revision from `WorldPose`'s subtree walk, at each static node it stamps
/// (`stamp.rs`); the walk runs only while a cursor is open on that channel, and is
/// skipped for a subtree still unread since the newest read of any of its cursors.
/// So the cursor must be caught up every time the revision is compared: one that
/// lagged would let a second move of a subtree already counted go unbumped. Asking
/// does both, so the contract cannot be half-kept.
pub struct StaticGeometry {
    pose: Cursor<WorldPose>,
    seen: u64,
}

impl StaticGeometry {
    /// Whether anything changed since the last ask (or the watch opened); `true`
    /// too when the cursor reads [`Changes::Rebuild`] (a clear, an Undo step, a
    /// rewind, another World). Then everything up to now has been seen.
    pub fn changed(&mut self, w: &World) -> bool {
        let held = w.read(&mut self.pose, |log, seen| log.skip(seen).then_some(()));
        let now = w.changes.borrow().static_geometry;
        let moved = std::mem::replace(&mut self.seen, now) != now;
        held.is_none() || moved
    }
}
