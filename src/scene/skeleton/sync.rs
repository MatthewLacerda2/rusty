//! Spawning and re-binding a skinned mesh's bone GameObjects (#453).

use std::collections::BTreeSet;

use glam::Mat4;

use super::bone_refs::BoneRef;
use crate::asset::SkinData;
use crate::components::mesh::BoneBody;
use crate::components::TransformComponent;
use crate::scene::Scene;

impl Scene {
    /// Make sure every skinned mesh has its skeleton (see [`Scene::sync_skeleton`]).
    /// Idempotent and cheap when nothing changed; called after a scene load, a
    /// prefab stamp and an asset instantiate.
    pub fn sync_skeletons(&mut self) {
        let mut refs = Vec::new();
        for owner in self.world.ids_with_mesh() {
            refs.extend(self.sync_skeleton_past(owner, &BTreeSet::new()));
        }
        self.bind_bone_refs(refs);
    }

    /// Bind `owner`'s skin to its bone entities, spawning the missing ones. Each
    /// joint slot is looked up by name under its parent bone (the skinned entity
    /// for a skeleton root) and spawned at its rest pose when absent — so a fresh
    /// instance gets a whole skeleton, and a mesh whose binding was reset (a prefab
    /// propagation rebuilt it) re-binds to the bones it already has. Then any
    /// pending per-bone overrides and bone bodies (#466) are applied by name; one
    /// whose bone no longer exists is dropped with a warning.
    pub fn sync_skeleton(&mut self, owner: u32) {
        let refs = self.sync_skeleton_past(owner, &BTreeSet::new());
        self.bind_bone_refs(refs);
    }

    /// [`Scene::sync_skeleton`], never taking one of `parked` for a bone: they are
    /// attachments waiting under `owner` to hang from a bone again, and one named
    /// like a root joint is still not that joint. Returns the restored bone joints
    /// tied to another skeleton's bone, for [`Scene::bind_bone_refs`] once every
    /// skeleton is built.
    pub(super) fn sync_skeleton_past(
        &mut self,
        owner: u32,
        parked: &BTreeSet<u32>,
    ) -> Vec<BoneRef> {
        let Some((skin, bound)) = self
            .world
            .mesh(owner)
            .and_then(|m| Some((m.skin.clone()?, m.skeleton.bones.clone())))
        else {
            return Vec::new();
        };
        let bones = if self.binding_is_live(&skin, &bound) {
            bound
        } else {
            self.bind_bones(owner, &skin, parked)
        };
        let (overrides, bodies) = match self.world.mesh_mut(owner) {
            Some(mut mesh) => {
                mesh.skeleton.bones = bones.clone();
                (
                    std::mem::take(&mut mesh.skeleton.overrides),
                    std::mem::take(&mut mesh.skeleton.bodies),
                )
            }
            None => return Vec::new(),
        };
        for (name, transform) in overrides {
            match self.find_bone(owner, &name) {
                Some(bone) => {
                    if let Some(mut t) = self.world.transform_mut(bone) {
                        *t = transform;
                    }
                }
                None => log::warn!(
                    "[Asset] entity {owner}: the skeleton has no bone '{name}' any more; \
                     its saved transform override was dropped."
                ),
            }
        }
        let mut refs = Vec::new();
        for (name, body) in bodies {
            refs.extend(self.restore_bone_body(owner, &name, body));
        }
        refs
    }

    /// Put a saved [`BoneBody`] back on `owner`'s bone `name`, its joint's
    /// connected bone of the same skeleton resolved by its chain, as a live
    /// re-import resolves it (#1277). One of another skeleton is returned, to bind
    /// once that skeleton is built.
    fn restore_bone_body(&mut self, owner: u32, name: &str, body: BoneBody) -> Vec<BoneRef> {
        let Some(bone) = self.find_bone(owner, name) else {
            log::warn!(
                "[Asset] entity {owner}: the skeleton has no bone '{name}' any more; \
                 its saved Rigidbody / Joint were dropped."
            );
            return Vec::new();
        };
        let other = body.joint.as_ref().and_then(|j| j.connected_body);
        let refs = body
            .bone_refs
            .into_iter()
            .filter_map(|(pointer, chain)| {
                let owner = other?;
                Some(BoneRef {
                    holder: bone,
                    pointer,
                    owner,
                    chain,
                })
            })
            .collect();
        let mut joint = body.joint;
        if let Some(j) = joint.as_mut().filter(|_| !body.connected_bone.is_empty()) {
            let to = self.resolve_bone(bone, owner, &body.connected_bone);
            if to == bone || to == owner {
                let what = if to == bone {
                    "itself"
                } else {
                    "its skinned entity"
                };
                log::warn!(
                    "[Asset] entity {owner}: bone '{name}''s joint now connects it to \
                     {what}; the joint is inert."
                );
            }
            j.connected_body = Some(to);
        }
        self.world.set_rigidbody(bone, body.rigidbody);
        self.world.set_joint(bone, joint);
        refs
    }

    /// Whether `bones` is a complete binding of `skin`: one slot per joint, each
    /// a live entity or missing (a destroyed bone stays missing, #1273). A bone a
    /// script re-parented elsewhere is still live and stays bound: the palette
    /// skins it by its world matrix, as Unity does.
    fn binding_is_live(&self, skin: &SkinData, bones: &[Option<u32>]) -> bool {
        let live = |b: &u32| self.world.contains(*b);
        bones.len() == skin.local_bind.len() && bones.iter().flatten().all(live)
    }

    /// Walk the joint slots in order (parents first), finding each bone by name
    /// under its parent (never one of `parked`) or spawning it at rest. Returns the
    /// slot → entity binding.
    fn bind_bones(
        &mut self,
        owner: u32,
        skin: &SkinData,
        parked: &BTreeSet<u32>,
    ) -> Vec<Option<u32>> {
        let mut taken: BTreeSet<u32> = BTreeSet::new();
        self.walk_slots(owner, skin, |scene, slot, parent| {
            let name = skin.name(slot);
            let existing = scene.world.children(parent).into_iter().find(|&c| {
                !taken.contains(&c)
                    && !parked.contains(&c)
                    && scene.world.name(c).is_some_and(|n| *n == name)
            });
            let bone =
                existing.unwrap_or_else(|| scene.spawn_bone(parent, name, rest_local(skin, slot)));
            taken.insert(bone);
            bone
        })
    }

    /// Bind `skin`'s joint slots in order, parents first, each to the bone `pick`
    /// gives it under its parent's bone (`owner` for a skeleton root, or for a
    /// parent link that does not point to an earlier slot). Returns the slot →
    /// entity binding: the one walk a bind and a re-import share.
    pub(super) fn walk_slots(
        &mut self,
        owner: u32,
        skin: &SkinData,
        mut pick: impl FnMut(&mut Scene, usize, u32) -> u32,
    ) -> Vec<Option<u32>> {
        let mut bones: Vec<u32> = Vec::with_capacity(skin.local_bind.len());
        for slot in 0..skin.local_bind.len() {
            let parent = match skin.parents.get(slot).copied().flatten() {
                Some(p) if p < slot => bones[p],
                _ => owner,
            };
            bones.push(pick(self, slot, parent));
        }
        bones.into_iter().map(Some).collect()
    }

    pub(super) fn spawn_bone(
        &mut self,
        parent: u32,
        name: String,
        rest: TransformComponent,
    ) -> u32 {
        let id = self.add_entity(name);
        if let Some(mut t) = self.world.transform_mut(id) {
            *t = rest;
        }
        let _ = self.set_parent(id, Some(parent));
        id
    }
}

/// A bone's rest-pose local transform relative to its parent entity: the joint's
/// bind local, or for a skeleton root its bind pose in the skinned entity's space
/// (which folds in any `Armature` node between the mesh and the root).
pub(super) fn rest_local(skin: &SkinData, slot: usize) -> TransformComponent {
    let is_root = skin.parents.get(slot).copied().flatten().is_none();
    match (is_root, skin.local_bind.get(slot)) {
        (false, Some(local)) => TransformComponent {
            position: local.translation,
            rotation: local.rotation,
            scale: local.scale,
        },
        _ => TransformComponent::from_matrix(
            skin.bind_global
                .get(slot)
                .copied()
                .unwrap_or(Mat4::IDENTITY),
        ),
    }
}
