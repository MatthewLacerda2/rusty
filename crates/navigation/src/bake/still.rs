//! crates/navigation/src/bake/still.rs — whether the static geometry the bake reads changed
//! since the last play tick, from the World's change log (#1707).
//!
//! The bake's colliders are the static ones (`bounds::static_collider_ids`): an
//! entity static and active in the hierarchy, with an active, solid Collider, at
//! its world pose. The World answers each of those facts without a list of ids:
//! - a static entity's pose or Collider: [`World::static_geometry_revision`], which
//!   the `WorldPose` walk and a Collider write bump when the entity is static. The
//!   watch keeps a `WorldPose` cursor it only catches up ([`World::catch_up`]), so
//!   the walk runs and the skip that spares a subtree already stamped never spares
//!   one this watch has not seen;
//! - `activeInHierarchy` and static membership: `StaticSet`, which every Activity
//!   stamp also stamps for each static entity it reaches (an `active` flag flipped,
//!   a re-parent, a placement or removal), and which names a removed static entity.
//!
//! A `NavMeshModifier` (#1959) changes how they bake from anywhere above them, so
//! any write on its channel counts as a change.
//! When the revision is the one last seen, `StaticSet` named nothing and no cursor
//! read `Rebuild` (a clear, a scene load, Play's snapshot restored, an Undo or a
//! rewind), the collider diff, the bounds' geometry extent and the reshape check
//! would all find what they found last tick, so [`NavigationGraph::sync_changes`]
//! skips them. A dynamic entity moving costs the watch nothing per entity.

use super::super::NavigationGraph;
use super::sync::Rebake;
use rusty_components::NavMeshModifierComponent;
use rusty_ecs::{Changes, Cursor, StaticSet, World, WorldPose};
use rusty_scene::Scene;

/// What a play-mode navmesh reads its static geometry's changes from.
pub(crate) struct StaticWatch {
    /// Caught up, never read: it keeps `WorldPose`'s walks (and so the revision's
    /// pose bumps) running, and reads `false` on a `Rebuild`.
    pose: Cursor<WorldPose>,
    geometry: u64,
    statics: Cursor<StaticSet>,
    modifiers: Cursor<NavMeshModifierComponent>,
}

impl StaticWatch {
    fn open(w: &mut World) -> Self {
        Self {
            pose: w.change_cursor(),
            geometry: w.static_geometry_revision(),
            statics: w.change_cursor(),
            modifiers: w.change_cursor(),
        }
    }

    /// Whether the static geometry is as it was at the last read. Reads every
    /// cursor, so each has read everything after this.
    pub(crate) fn still(&mut self, w: &mut World) -> bool {
        let unchanged = |c: Changes| matches!(c, Changes::Ids(ids) if ids.is_empty());
        let membership = unchanged(w.changes(&mut self.statics));
        // Any `NavMeshModifier` write counts: one on a non-static parent changes
        // how its static children bake (#1959). They are rare.
        let modifiers = unchanged(w.changes(&mut self.modifiers));
        let pose = w.catch_up(&mut self.pose);
        let geometry = std::mem::replace(&mut self.geometry, w.static_geometry_revision());
        membership && modifiers && pose && geometry == self.geometry
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
        self.start_tracked_with(scene, None)
    }

    /// [`Self::start_tracked`] for a load that baked the navmesh off the ticking
    /// thread (`app::scene_load::background`, #2123): with no navmesh saved, the
    /// `baked` bytes are restored when they still match the live scene, the same
    /// graph a bake gives; otherwise it bakes, as [`Self::restore_or_bake`].
    pub fn start_tracked_with(&mut self, scene: &mut Scene, baked: Option<&[u8]>) -> Start {
        let watch = StaticWatch::open(&mut scene.world);
        let start = match baked.filter(|_| scene.navmesh.bytes.is_none()) {
            Some(bytes) if self.restore(scene, bytes).is_ok() => Start::Restored,
            _ => self.restore_or_bake(scene),
        };
        self.static_watch = Some(watch);
        self.tracked_scene = Some((scene.id(), start));
        start
    }

    /// A full bake of `scene` by a fresh graph, as the bytes that restore it: what a
    /// load bakes from its files when no navmesh is saved beside them.
    pub fn baked_bytes(scene: &Scene) -> Vec<u8> {
        let b = crate::NavBounds::EMPTY_SCENE;
        let spacing = rusty_scene::nav_settings::DEFAULT_GRID_SPACING;
        Self::new(b.min_x, b.max_x, b.min_z, b.max_z, spacing).bake_to_bytes(scene)
    }

    /// Whether [`Self::start_tracked`] last started on `scene` (a play-mode
    /// `Scene.Load` swaps in another, which starts afresh).
    pub fn tracks(&self, scene: &Scene) -> bool {
        self.tracked_scene.is_some_and(|(id, _)| id == scene.id())
    }

    /// How the last [`Self::start_tracked`] made the navmesh: restored, so no bake
    /// ran on that tick, or baked; `None` before the first.
    pub fn last_start(&self) -> Option<Start> {
        self.tracked_scene.map(|(_, start)| start)
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
