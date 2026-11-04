//! src/navigation/bake/still.rs — whether the static geometry the bake reads changed
//! since the last play tick, from the World's change log (#1707).
//!
//! The bake's colliders are the static ones (`bounds::static_collider_ids`): an
//! entity static and active in the hierarchy, with an active, solid Collider, at
//! its world pose. Each of those facts has a channel: a pose on `WorldPose`, the
//! Collider on `ColliderComponent`, `activeInHierarchy` on `Activity`, and static
//! membership on `StaticSet`, which also names a static entity that was removed.
//! When no id on the first three is static now and `StaticSet` named nothing, the
//! collider diff, the bounds' geometry extent and the reshape check would all find
//! what they found last tick, so [`NavigationGraph::sync_changes`] skips them. A
//! dynamic entity moving names no static id, so it never costs the static diff.

use super::super::NavigationGraph;
use super::sync::Rebake;
use crate::components::ColliderComponent;
use crate::ecs::{Activity, Changes, Channel, Cursor, StaticSet, World, WorldPose};
use crate::scene::Scene;

/// The cursors a play-mode navmesh reads its static geometry's changes from.
pub(in crate::navigation) struct StaticWatch {
    pose: Cursor<WorldPose>,
    collider: Cursor<ColliderComponent>,
    activity: Cursor<Activity>,
    statics: Cursor<StaticSet>,
}

/// Whether `cursor` named no static entity (`false` on `Rebuild`).
fn none_static<T: Channel>(w: &mut World, cursor: &mut Cursor<T>) -> bool {
    match w.changes(cursor) {
        Changes::Rebuild => false,
        Changes::Ids(ids) => !ids.iter().any(|&id| w.is_static(id)),
    }
}

impl StaticWatch {
    fn open(w: &mut World) -> Self {
        Self {
            pose: w.change_cursor(),
            collider: w.change_cursor(),
            activity: w.change_cursor(),
            statics: w.change_cursor(),
        }
    }

    /// Whether the static geometry is as it was at the last read. Reads every
    /// cursor, so each has read everything after this.
    pub(in crate::navigation) fn still(&mut self, w: &mut World) -> bool {
        let membership =
            matches!(w.changes(&mut self.statics), Changes::Ids(ids) if ids.is_empty());
        let reads = [
            none_static(w, &mut self.pose),
            none_static(w, &mut self.collider),
            none_static(w, &mut self.activity),
        ];
        membership && !reads.contains(&false)
    }
}

impl NavigationGraph {
    /// [`Self::sync`] for the play loop, which calls it every tick: when the
    /// change log says no static entity changed since the last call, the static
    /// collider diff and the bounds check are skipped (obstacles, modifier
    /// volumes and authored links are still diffed). The first call, a scene
    /// load, an Undo or any `Rebuild` runs the full [`Self::sync`].
    pub fn sync_changes(&mut self, scene: &mut Scene) -> Rebake {
        let still = match self.static_watch.as_mut() {
            Some(watch) => watch.still(&mut scene.world),
            None => {
                self.static_watch = Some(StaticWatch::open(&mut scene.world));
                false
            }
        };
        let watch = self.static_watch.take();
        let rebake = self.sync_inner(scene, still);
        // A bake that re-shaped the grid made a new graph; the watch carries over.
        self.static_watch = watch;
        rebake
    }

    /// [`Self::bake`] for the play loop's first tick, opening the watch
    /// [`Self::sync_changes`] reads from then on.
    pub fn bake_tracked(&mut self, scene: &mut Scene) {
        let watch = StaticWatch::open(&mut scene.world);
        self.bake(scene);
        self.static_watch = Some(watch);
    }
}
