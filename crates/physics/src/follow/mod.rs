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
//! bone is found by walking every skeleton's subtrees ([`Scene::bone_subtrees`]),
//! and is kept between ticks with the bones the skeletons list. The walk runs
//! again only when the follow's own cursors say the hung set may have changed:
//! - `MeshComponent`: a `Rebuild`, or a stamped id whose skeleton's bone slots
//!   differ from the kept ones (it became a skinned owner, stopped being one, or
//!   a skeleton was bound, re-imported or had a slot cleared), which changes
//!   which ids are bones. Any other write to a Mesh (a decal's, a muzzle flash's,
//!   a script setting a character's draw distance) walks nothing, and the palette
//!   written every tick (`World::set_pose_palette`) stamps nothing, so an
//!   animating rig keeps its list. A prefab stamp syncs every skeleton
//!   (`Scene::hang_on_skeletons`), and one already bound is left unwritten.
//! - `Activity`: a `Rebuild`, or a stamped id that touches a rig. A re-parent, a
//!   placement or a removal stamps the whole subtree (`World::stamp_hierarchy_below`,
//!   `World::touch_entity`), so its top is stamped too: a stamped id touches a rig
//!   when it was hung, or when it or an ancestor is hung or listed as a bone now.
//!   A casing spawned at the root (a prefab's included), a projectile destroyed
//!   or a prop toggled touches none, so a busy tick away from the rigs walks
//!   nothing; an edit on or under a rig (a hitbox moved, a bone deactivated, a
//!   prefab stamped under a bone) walks once.
//!
//! The list itself is the hung ids that have a collider handle. When an id gained
//! or lost one since (`ColliderIds::generation`: a Collider attached, detached or
//! toggled, a mesh collider built or dropped), the kept hung set is filtered again,
//! without a walk; a handle only replaced (a body plan rebuilt, a projectile's
//! among them) moves nothing, since each tick looks the handle up. A scene
//! load, Play's restore and an Undo or Redo read `Rebuild` on every cursor, and a
//! fresh physics world (entering Play, a caught rapier panic) starts with no list.
//! Each tick still looks the handle up and skips a body's own collider (a
//! Rigidbody added to a static hitbox keeps its plan, so no walk sees it). A quiet
//! tick therefore writes exactly what the walk would: the same colliders,
//! ascending, so the refit set and the next step's query tree are the walk's.

#[cfg(test)]
mod cost_tests;
#[cfg(test)]
mod edit_ops;
#[cfg(test)]
mod narrow_tests;
#[cfg(test)]
mod prop_tests;

use std::cell::Cell;

use rapier3d::prelude::Collider;

use super::collider_ids::ColliderIds;
use super::compound::world_pose;
use super::convert::to_pose;
use super::world::{collider_to_write, PhysicsWorld};
use rusty_components::MeshComponent;
use rusty_ecs::{Activity, Changes, Cursor};
use rusty_scene::Scene;

/// The bone-hung colliders kept between follows, and the cursors that say when
/// to walk for them again.
pub(crate) struct Kept {
    activity: Cursor<Activity>,
    mesh: Cursor<MeshComponent>,
    /// `ColliderIds::generation` when `ids` was filtered.
    generation: u64,
    /// Every id on or under a bone, ascending: the walk's answer.
    hung: Vec<u32>,
    /// Every id a skeleton's slots list, ascending, under its owner or not.
    bones: Vec<u32>,
    /// The skinned owners (`Scene::skinned_ids`), ascending, each with the bone
    /// slots its skeleton held: the hung set reads nothing else of a Mesh.
    owners: Vec<(u32, Vec<Option<u32>>)>,
    /// `hung`'s ids with a collider handle.
    ids: Vec<u32>,
    /// Walks taken since this list was opened, for the cost tests.
    #[cfg(test)]
    pub walks: u64,
}

impl Kept {
    /// Open the cursors and walk once.
    fn open(scene: &Scene, handles: &ColliderIds) -> Self {
        let w = &scene.world;
        let mut kept = Self {
            activity: w.change_cursor(),
            mesh: w.change_cursor(),
            generation: handles.generation(),
            hung: Vec::new(),
            bones: Vec::new(),
            owners: Vec::new(),
            ids: Vec::new(),
            #[cfg(test)]
            walks: 0,
        };
        kept.walk(scene, handles);
        kept
    }

    /// Walk again when a cursor says the hung set may have changed (the module
    /// docs), else filter again when the handles did. Every cursor is read either
    /// way, so each has read everything after this.
    fn refresh(&mut self, scene: &Scene, handles: &ColliderIds) {
        let w = &scene.world;
        let skins = match w.changes(&mut self.mesh) {
            Changes::Rebuild => true,
            Changes::Ids(ids) => ids.iter().any(|&id| self.skins(scene, id)),
        };
        let touched = match w.changes(&mut self.activity) {
            Changes::Rebuild => true,
            Changes::Ids(ids) => ids.iter().any(|&id| self.touches(scene, id)),
        };
        if skins || touched {
            self.walk(scene, handles);
        } else if handles.generation() != self.generation {
            refilter(&mut self.ids, &self.hung, handles);
        }
        self.generation = handles.generation();
    }

    /// Whether Mesh-stamped `id` can have changed which ids are bones: it is a
    /// skinned owner now and was not, was one and is not, or its bone slots
    /// moved. Any other field written (a script's `Mesh.SetMaxDrawDistance`) walks
    /// nothing.
    fn skins(&self, scene: &Scene, id: u32) -> bool {
        let was = self.owners.binary_search_by_key(&id, |(o, _)| *o).ok();
        let was = was.map(|at| self.owners[at].1.as_slice());
        let mesh = scene.world.mesh(id);
        let now = mesh
            .as_deref()
            .map(|m: &MeshComponent| m.skeleton.bones.as_slice());
        let now = now.filter(|slots| !slots.is_empty());
        was != now
    }

    /// Whether stamped `id` can have changed the hung set: it was hung, or it or
    /// an ancestor is hung or listed as a bone now. A removed id has no
    /// ancestors left; it mattered only if it was hung.
    fn touches(&self, scene: &Scene, id: u32) -> bool {
        let kept =
            |a: u32| self.hung.binary_search(&a).is_ok() || self.bones.binary_search(&a).is_ok();
        let w = &scene.world;
        if !w.contains(id) {
            return self.hung.binary_search(&id).is_ok();
        }
        w.self_and_ancestors(id).any(kept)
    }

    /// Find the hung set and the bones again, and the list from them.
    fn walk(&mut self, scene: &Scene, handles: &ColliderIds) {
        self.hung = scene.bone_subtrees().into_iter().collect();
        let mut owners = scene.skinned_ids();
        owners.sort_unstable();
        self.bones = listed_bones(scene, &owners);
        let slots = |o: u32| scene.world.mesh(o).map(|m| m.skeleton.bones.clone());
        self.owners = owners
            .into_iter()
            .map(|o| (o, slots(o).unwrap_or_default()))
            .collect();
        refilter(&mut self.ids, &self.hung, handles);
        #[cfg(test)]
        {
            self.walks += 1;
        }
    }

    /// The kept ids, ascending.
    #[cfg(test)]
    pub(crate) fn ids(&self) -> &[u32] {
        &self.ids
    }
}

/// Every id the skeleton slots of `owners` list, ascending, each once.
fn listed_bones(scene: &Scene, owners: &[u32]) -> Vec<u32> {
    let mut bones = Vec::new();
    for &owner in owners {
        if let Some(mesh) = scene.world.mesh(owner) {
            bones.extend(mesh.skeleton.bones.iter().flatten().copied());
        }
    }
    bones.sort_unstable();
    bones.dedup();
    bones
}

/// `hung`'s ids that have a collider handle, ascending.
fn filtered(hung: &[u32], handles: &ColliderIds) -> Vec<u32> {
    let mut ids = Vec::new();
    refilter(&mut ids, hung, handles);
    ids
}

/// Fill `ids` with [`filtered`]'s answer, reusing its buffer.
fn refilter(ids: &mut Vec<u32>, hung: &[u32], handles: &ColliderIds) {
    ids.clear();
    ids.extend(hung.iter().copied().filter(|id| handles.contains_key(id)));
}

/// What the kept list holds, by the walk: every id on or under a bone that has a
/// collider handle, ascending.
fn walked(scene: &Scene, handles: &ColliderIds) -> Vec<u32> {
    let hung: Vec<u32> = scene.bone_subtrees().into_iter().collect();
    filtered(&hung, handles)
}

thread_local! {
    static CHECKED: Cell<bool> = const { Cell::new(false) };
}

/// Have every follow on this thread also walk the skeletons and panic when the
/// kept list differs from the walk's. A test hook: the hitbox and ragdoll suites
/// arm it, so every tick they run holds the kept list to the walk. A build
/// without `debug_assertions` (a shipped game) never reads it.
#[doc(hidden)]
pub fn check_kept_bones() {
    CHECKED.with(|c| c.set(true));
}

/// Whether this follow walks for a reference: always in a twin built to
/// (`Tracked::always_walk`, tests only), or when the check above is armed.
fn walks_too(physics: &PhysicsWorld) -> bool {
    #[cfg(test)]
    if physics.tracked.always_walk {
        return true;
    }
    let _ = physics;
    cfg!(debug_assertions) && CHECKED.with(Cell::get)
}

impl PhysicsWorld {
    /// Re-place each collider on or under a bone at its live world pose and refit
    /// the query pipeline over the ones that moved. Visits the kept list
    /// (above), walking the skeletons' subtrees only when an edit touched a rig
    /// since the last follow, never the scene's other colliders, so a map's
    /// static colliders cost nothing here (#1617).
    pub fn follow_bones(&mut self, scene: &Scene) {
        let kept = match self.tracked.follow.take() {
            Some(mut k) => {
                k.refresh(scene, &self.id_to_collider);
                k
            }
            None => Kept::open(scene, &self.id_to_collider),
        };
        let walk = walks_too(self).then(|| walked(scene, &self.id_to_collider));
        if let Some(walk) = &walk {
            assert_eq!(&kept.ids, walk, "the kept bone colliders went stale");
        }
        self.place_on_bones(scene, walk.as_deref().unwrap_or(&kept.ids));
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
