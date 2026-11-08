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

    /// The play loop's first tick, and its first after a play-mode scene load: the
    /// scene's saved navmesh (`Scene::navmesh`, #1887) when it still matches the
    /// scene ([`Self::restore`]), else a full [`Self::bake`], with one warning when a
    /// saved one was refused. Opens the watch [`Self::sync_changes`] reads from then
    /// on, and remembers which scene it started on ([`Self::tracks`]).
    pub fn start_tracked(&mut self, scene: &mut Scene) -> Start {
        let watch = StaticWatch::open(&mut scene.world);
        let start = match scene.navmesh.clone() {
            Some(bytes) => match self.restore(scene, &bytes) {
                Ok(()) => Start::Restored,
                Err(why) => {
                    log::warn!("[Navigation] the saved navmesh was refused ({why}): baking it");
                    self.bake(scene);
                    Start::Baked
                }
            },
            None => {
                self.bake(scene);
                Start::Baked
            }
        };
        self.static_watch = Some(watch);
        self.tracked_scene = Some(scene.id());
        start
    }

    /// Whether [`Self::start_tracked`] last started on `scene` (a play-mode
    /// `Scene.Load` swaps in another, which starts afresh).
    pub fn tracks(&self, scene: &Scene) -> bool {
        self.tracked_scene == Some(scene.id())
    }
}

/// How [`NavigationGraph::start_tracked`] made the navmesh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// From the scene's saved navmesh: no bake ran.
    Restored,
    /// By a full bake.
    Baked,
}
