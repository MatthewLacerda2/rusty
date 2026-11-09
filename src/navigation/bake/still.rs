//! src/navigation/bake/still.rs — whether the static geometry the bake reads changed
//! since the last play tick, from the World's change log (#1707).
//!
//! The bake's colliders are the static ones (`bounds::static_collider_ids`): an
//! entity static and active in the hierarchy, with an active, solid Collider, at
//! its world pose. Each of those facts has a channel: a pose on `WorldPose`, the
//! Collider on `ColliderComponent`, `activeInHierarchy` on `Activity`, and static
//! membership on `StaticSet`, which also names a static entity that was removed.
//! A `NavMeshModifier` (#1959) changes how they bake from anywhere above them, so
//! any write on its channel counts as a change.
//! When no id on the first three is static now and `StaticSet` named nothing, the
//! collider diff, the bounds' geometry extent and the reshape check would all find
//! what they found last tick, so [`NavigationGraph::sync_changes`] skips them. A
//! dynamic entity moving names no static id, so it never costs the static diff.

use super::super::NavigationGraph;
use super::sync::Rebake;
use crate::components::{ColliderComponent, NavMeshModifierComponent};
use crate::ecs::{Activity, Changes, Channel, Cursor, StaticSet, World, WorldPose};
use crate::scene::Scene;

/// The cursors a play-mode navmesh reads its static geometry's changes from.
pub(in crate::navigation) struct StaticWatch {
    pose: Cursor<WorldPose>,
    collider: Cursor<ColliderComponent>,
    activity: Cursor<Activity>,
    statics: Cursor<StaticSet>,
    modifiers: Cursor<NavMeshModifierComponent>,
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
            modifiers: w.change_cursor(),
        }
    }

    /// Whether the static geometry is as it was at the last read. Reads every
    /// cursor, so each has read everything after this.
    pub(in crate::navigation) fn still(&mut self, w: &mut World) -> bool {
        let unchanged = |c: Changes| matches!(c, Changes::Ids(ids) if ids.is_empty());
        let membership = unchanged(w.changes(&mut self.statics));
        // Any `NavMeshModifier` write counts: one on a non-static parent changes
        // how its static children bake (#1959). They are rare.
        let modifiers = unchanged(w.changes(&mut self.modifiers));
        let reads = [
            none_static(w, &mut self.pose),
            none_static(w, &mut self.collider),
            none_static(w, &mut self.activity),
        ];
        membership && modifiers && !reads.contains(&false)
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

    /// The navmesh of a scene as it is loaded (#1887): its saved navmesh
    /// (`Scene::navmesh`) when it still matches the scene ([`Self::restore`]), else
    /// a full [`Self::bake`], with one warning when a saved one was refused. The one
    /// way a loaded scene gets its navmesh: boot ([`Self::from_scene`]), Play's first
    /// tick and a play-mode `Scene.Load` ([`Self::start_tracked`]), and Stop.
    pub fn restore_or_bake(&mut self, scene: &Scene) -> Start {
        let Some(bytes) = scene.navmesh.bytes.clone() else {
            self.bake(scene);
            return Start::Baked;
        };
        match self.restore(scene, &bytes) {
            Ok(()) => Start::Restored,
            Err(why) => {
                log::warn!("[Navigation] the saved navmesh was refused ({why}): baking it");
                self.bake(scene);
                Start::Baked
            }
        }
    }

    /// The play loop's first tick, and a play-mode scene load (`app::scene_load`):
    /// [`Self::restore_or_bake`], then open the watch [`Self::sync_changes`] reads
    /// from, and remember which scene it started on ([`Self::tracks`]).
    pub fn start_tracked(&mut self, scene: &mut Scene) -> Start {
        let watch = StaticWatch::open(&mut scene.world);
        let start = self.restore_or_bake(scene);
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
