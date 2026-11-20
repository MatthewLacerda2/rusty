//! crates/navigation/src/bake/still.rs — whether the static geometry the bake reads changed
//! since the last play tick, from the World's change log (#1707).
//!
//! The bake's colliders are the static ones (`bounds::static_collider_ids`): an
//! entity static and active in the hierarchy, with an active, solid Collider, at
//! its world pose. The World answers each of those facts without a list of ids:
//! - a static entity's pose or Collider: a [`StaticGeometry`] watch, a revision the
//!   `WorldPose` walk and a Collider write bump when the entity is static, held
//!   with the `WorldPose` cursor it catches up on every ask (so the walk runs, and
//!   the skip that spares a subtree already stamped never spares one this watch
//!   has not seen);
//! - `activeInHierarchy` and static membership: `StaticSet`, which every Activity
//!   stamp also stamps for each static entity it reaches (an `active` flag flipped,
//!   a re-parent, a placement or removal), and which names a removed static entity.
//!
//! A `NavMeshModifier` (#1959) changes how they bake from anywhere above them, so a
//! change of one counts wherever it is. A stamp selects, a value decides: an id its
//! channel names counts when its modifier now differs from the one kept at the last
//! read (one added, removed or edited), so a modifier's unchanged rewrite costs a
//! comparison, not the collider diff.
//! When the watch saw nothing, `StaticSet` named nothing and no cursor read
//! `Rebuild` (a clear, a scene load, Play's snapshot restored, an Undo or a
//! rewind), the collider diff, the bounds' geometry extent and the reshape check
//! would all find what they found last tick, so [`NavigationGraph::sync_changes`]
//! skips them. A dynamic entity moving costs the watch no id list and no `is_static`
//! lookup per id; its move still walks its subtree and appends its `WorldPose`
//! entries, which the open cursor keeps recording.

use super::super::NavigationGraph;
use super::sync::Rebake;
use rusty_components::NavMeshModifierComponent;
use rusty_core::collections::Map;
use rusty_ecs::{Changes, Cursor, StaticGeometry, StaticSet, World};
use rusty_scene::Scene;

/// What a play-mode navmesh reads its static geometry's changes from.
pub(crate) struct StaticWatch {
    geometry: StaticGeometry,
    statics: Cursor<StaticSet>,
    modifiers: Modifiers,
}

/// The `NavMeshModifier` channel's cursor and every modifier as it was at its last
/// read (they are rare: a few per level).
struct Modifiers {
    cursor: Cursor<NavMeshModifierComponent>,
    kept: Map<u32, NavMeshModifierComponent>,
}

impl Modifiers {
    fn open(w: &World) -> Self {
        let mut m = Self {
            cursor: w.change_cursor(),
            kept: Map::default(),
        };
        m.rebuild(w);
        m
    }

    fn rebuild(&mut self, w: &World) {
        self.kept.clear();
        for id in w.ids_with_nav_mesh_modifier() {
            if let Some(m) = w.nav_mesh_modifier(id) {
                self.kept.insert(id, m.clone());
            }
        }
    }

    /// Whether every modifier is as at the last read; each id the channel named is
    /// kept anew. A `Rebuild` is a change.
    fn still(&mut self, w: &World) -> bool {
        let ids = match w.changes(&mut self.cursor) {
            Changes::Rebuild => {
                self.rebuild(w);
                return false;
            }
            Changes::Ids(ids) => ids,
        };
        let mut still = true;
        for id in ids {
            let now = w.nav_mesh_modifier(id).map(|m| m.clone());
            if now.as_ref() == self.kept.get(&id) {
                continue;
            }
            still = false;
            match now {
                Some(m) => self.kept.insert(id, m),
                None => self.kept.remove(&id),
            };
        }
        still
    }
}

impl StaticWatch {
    fn open(w: &mut World) -> Self {
        Self {
            geometry: w.static_geometry(),
            statics: w.change_cursor(),
            modifiers: Modifiers::open(w),
        }
    }

    /// Whether the static geometry is as it was at the last read. Reads every
    /// cursor, so each has read everything after this.
    pub(crate) fn still(&mut self, w: &mut World) -> bool {
        let membership = w.changes(&mut self.statics).is_empty();
        // A modifier changed anywhere counts: one on a non-static parent changes
        // how its static children bake (#1959).
        let modifiers = self.modifiers.still(w);
        let geometry = self.geometry.changed(w);
        membership && modifiers && !geometry
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
