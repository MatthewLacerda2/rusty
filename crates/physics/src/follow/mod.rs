//! crates/physics/src/follow/ — colliders that follow animated bones (#464).
//!
//! A tick runs `Update` scripts → the physics step → the Animator → `LateUpdate`.
//! The step pushes the poses that changed into rapier at its *start* (#1707), so the bones the
//! Animator (and `LateUpdate`) moved afterwards would otherwise be queried where
//! they stood a tick earlier — a headshot would hit last frame's head. After the
//! tick's last bone writer, [`PhysicsWorld::follow_bones`] moves every collider
//! that hangs from a bone to its bone's current pose and refits the query
//! pipeline, so the next tick's casts test the pose that was rendered.
//!
//! Only query poses move here: bodies are untouched (the next step still drives
//! them, kinematic velocities included), and a body's own collider — a
//! Rigidbody's or a CharacterController's — is the step's to place.
//!
//! **The kept list** ([`Kept`], on `watch::Tracked`). Which entities hang from a
//! bone and carry a collider is found by walking every skeleton's subtrees
//! ([`Scene::bone_subtrees`]), and is kept between ticks: the walk runs again only
//! when the follow's own cursors read an id or a `Rebuild` since its last run, on
//! - `Activity`: a re-parent, a placement or a removal stamps the subtree
//!   (`World::stamp_hierarchy_below`, `World::touch_entity`), so a child spawned
//!   under a bone, a hitbox moved off one or a bone destroyed shows here;
//! - `MeshComponent`: a skeleton bound or re-imported, a bone slot cleared. The
//!   palette written every tick (`World::set_pose_palette`) stamps nothing, so an
//!   animating rig keeps its list.
//!
//! It walks again, too, when the colliders rapier holds changed since
//! (`Tracked::handles`: a body plan rebuilt, a mesh collider rebuilt or dropped,
//! #2059). That is what a Collider attached, detached or toggled changes, so
//! no cursor on `ColliderComponent` is needed: the list keeps ids by handle. A scene load, Play's restore and an Undo or Redo read `Rebuild` on
//! every cursor, and a fresh physics world (entering Play, a caught rapier panic)
//! starts with no list. The list keeps every hung id with a collider handle;
//! each tick still looks the handle up and skips a body's own collider (a
//! Rigidbody added to a static hitbox keeps its plan, so no walk sees it). A
//! quiet tick therefore writes exactly what the walk would: the same colliders,
//! ascending, so the refit set and the next step's query tree are the walk's.

#[cfg(test)]
mod cost_tests;
#[cfg(test)]
mod edit_ops;
#[cfg(test)]
mod prop_tests;

use std::cell::Cell;

use rapier3d::prelude::{Collider, ColliderHandle};

use super::compound::world_pose;
use super::convert::to_pose;
use super::world::{collider_to_write, PhysicsWorld};
use rusty_components::MeshComponent;
use rusty_core::collections::Map;
use rusty_ecs::{Activity, Changes, Channel, Cursor, World};
use rusty_scene::Scene;

/// The bone-hung colliders kept between follows, and the cursors that say when
/// to walk for them again.
pub(crate) struct Kept {
    activity: Cursor<Activity>,
    mesh: Cursor<MeshComponent>,
    /// `Tracked::handles` at the last walk.
    handles: u64,
    /// Every id on or under a bone with a collider handle, ascending.
    ids: Vec<u32>,
    /// Walks taken since this list was opened, for the cost tests.
    #[cfg(test)]
    pub walks: u64,
}

impl Kept {
    /// Open the cursors and walk once.
    fn open(scene: &Scene, handles: &Map<u32, ColliderHandle>, generation: u64) -> Self {
        let w = &scene.world;
        Self {
            activity: w.change_cursor(),
            mesh: w.change_cursor(),
            handles: generation,
            ids: walked(scene, handles),
            #[cfg(test)]
            walks: 1,
        }
    }

    /// Walk again when any cursor read an id or a `Rebuild` since the last follow,
    /// or the collider handles changed (`generation`). Every cursor is read either
    /// way, so each has read everything after this.
    fn refresh(&mut self, scene: &Scene, handles: &Map<u32, ColliderHandle>, generation: u64) {
        let w = &scene.world;
        let stamped = [
            stamped(w, &mut self.activity),
            stamped(w, &mut self.mesh),
            std::mem::replace(&mut self.handles, generation) != generation,
        ];
        if stamped.contains(&true) {
            self.ids = walked(scene, handles);
            #[cfg(test)]
            {
                self.walks += 1;
            }
        }
    }

    /// The kept ids, ascending.
    #[cfg(test)]
    pub(crate) fn ids(&self) -> &[u32] {
        &self.ids
    }
}

/// Whether `cursor` read anything since its last read: an id or a `Rebuild`.
fn stamped<T: Channel>(world: &World, cursor: &mut Cursor<T>) -> bool {
    !matches!(world.changes(cursor), Changes::Ids(ids) if ids.is_empty())
}

/// What the kept list holds, by the walk: every id on or under a bone that has a
/// collider handle, ascending.
fn walked(scene: &Scene, handles: &Map<u32, ColliderHandle>) -> Vec<u32> {
    let hung = scene.bone_subtrees().into_iter();
    hung.filter(|id| handles.contains_key(id)).collect()
}

thread_local! {
    static CHECKED: Cell<bool> = const { Cell::new(false) };
}

/// Have every follow on this thread also walk the skeletons and panic when the
/// kept list differs from the walk's. A test hook: the hitbox and ragdoll suites
/// arm it, so every tick they run holds the kept list to the walk.
#[doc(hidden)]
pub fn check_kept_bones() {
    CHECKED.with(|c| c.set(true));
}

impl PhysicsWorld {
    /// Re-place each collider on or under a bone at its live world pose and refit
    /// the query pipeline over the ones that moved. Visits the kept list
    /// (above), walking the skeletons' subtrees only when the hierarchy, a
    /// collider or a skeleton changed since the last follow, never the scene's
    /// other colliders, so a map's static colliders cost nothing here (#1617).
    pub fn follow_bones(&mut self, scene: &Scene) {
        let generation = self.tracked.handles;
        let kept = match self.tracked.follow.take() {
            Some(mut k) => {
                k.refresh(scene, &self.id_to_collider, generation);
                k
            }
            None => Kept::open(scene, &self.id_to_collider, generation),
        };
        #[cfg(test)]
        let walk_every_tick = self
            .tracked
            .always_walk
            .then(|| walked(scene, &self.id_to_collider));
        #[cfg(not(test))]
        let walk_every_tick: Option<Vec<u32>> = None;
        if CHECKED.with(Cell::get) {
            let walk = walked(scene, &self.id_to_collider);
            assert_eq!(kept.ids, walk, "the kept bone colliders went stale");
        }
        let ids = walk_every_tick.as_deref().unwrap_or(&kept.ids);
        self.place_on_bones(scene, ids);
        self.tracked.follow = Some(kept);
    }

    /// Move each of `ids`' colliders (ascending) to its world pose, skipping a
    /// body's own, and refit the query tree over the ones that moved.
    fn place_on_bones(&mut self, scene: &Scene, ids: &[u32]) {
        let mut moved = Vec::new();
        for &id in ids {
            let Some(&handle) = self.id_to_collider.get(&id) else {
                continue;
            };
            let carries_body = self.plan.contains_key(&id)
                && (scene.world.has_rigidbody(id) || scene.world.has_character_controller(id));
            if carries_body {
                continue;
            }
            let Some(pose) = world_pose(scene, id) else {
                continue;
            };
            let target = to_pose(pose.pos, pose.rot);
            let differs = |c: &Collider| *c.position() != target;
            if let Some(collider) = collider_to_write(&mut self.colliders, handle, differs) {
                collider.set_position(target);
                moved.push(handle);
            }
        }
        if !moved.is_empty() {
            self.refresh_query_tree(&moved);
        }
    }
}
