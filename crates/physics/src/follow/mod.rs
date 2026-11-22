//! Animator (and `LateUpdate`) moved afterwards would otherwise be queried where
//! they stood a tick earlier — a headshot would hit last frame's head. After the
//! Only query poses move here: bodies are untouched (the next step still drives
//! them, kinematic velocities included), and a body's own collider — a
//! Rigidbody's or a CharacterController's — is the step's to place.

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

pub(crate) struct Kept {
    activity: Cursor<Activity>,
    mesh: Cursor<MeshComponent>,
    generation: u64,
    hung: Vec<u32>,
    bones: Vec<u32>,
    owners: Vec<(u32, Vec<Option<u32>>)>,
    ids: Vec<u32>,
    #[cfg(test)]
    pub walks: u64,
}

impl Kept {
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

    fn touches(&self, scene: &Scene, id: u32) -> bool {
        let kept =
            |a: u32| self.hung.binary_search(&a).is_ok() || self.bones.binary_search(&a).is_ok();
        let w = &scene.world;
        if !w.contains(id) {
            return self.hung.binary_search(&id).is_ok();
        }
        w.self_and_ancestors(id).any(kept)
    }

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

    #[cfg(test)]
    pub(crate) fn ids(&self) -> &[u32] {
        &self.ids
    }
}

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

fn filtered(hung: &[u32], handles: &ColliderIds) -> Vec<u32> {
    let mut ids = Vec::new();
    refilter(&mut ids, hung, handles);
    ids
}

fn refilter(ids: &mut Vec<u32>, hung: &[u32], handles: &ColliderIds) {
    ids.clear();
    ids.extend(hung.iter().copied().filter(|id| handles.contains_key(id)));
}

fn walked(scene: &Scene, handles: &ColliderIds) -> Vec<u32> {
    let hung: Vec<u32> = scene.bone_subtrees().into_iter().collect();
    filtered(&hung, handles)
}

thread_local! {
    static CHECKED: Cell<bool> = const { Cell::new(false) };
}

#[doc(hidden)]
pub fn check_kept_bones() {
    CHECKED.with(|c| c.set(true));
}

fn walks_too(physics: &PhysicsWorld) -> bool {
    #[cfg(test)]
    if physics.tracked.always_walk {
        return true;
    }
    let _ = physics;
    cfg!(debug_assertions) && CHECKED.with(Cell::get)
}

impl PhysicsWorld {
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
