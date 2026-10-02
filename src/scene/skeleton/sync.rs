//! Spawning and re-binding a skinned mesh's bone GameObjects (#453).

use glam::Mat4;

use crate::asset::SkinData;
use crate::components::TransformComponent;
use crate::scene::Scene;

impl Scene {
    /// Make sure every skinned mesh has its skeleton (see [`Scene::sync_skeleton`]).
    /// Idempotent and cheap when nothing changed; called after a scene load, a
    /// prefab stamp and an asset instantiate.
    pub fn sync_skeletons(&mut self) {
        for owner in self.world.ids_with_mesh() {
            self.sync_skeleton(owner);
        }
    }

    /// Bind `owner`'s skin to its bone entities, spawning the missing ones. Each
    /// joint slot is looked up by name under its parent bone (the skinned entity
    /// for a skeleton root) and spawned at its rest pose when absent — so a fresh
    /// instance gets a whole skeleton, and a mesh whose binding was reset (a prefab
    /// propagation rebuilt it) re-binds to the bones it already has. Then any
    /// pending per-bone overrides are applied by name; one whose bone no longer
    /// exists is dropped with a warning.
    pub fn sync_skeleton(&mut self, owner: u32) {
        let Some((skin, bound)) = self
            .world
            .mesh(owner)
            .and_then(|m| Some((m.skin.clone()?, m.skeleton.bones.clone())))
        else {
            return;
        };
        let bones = if self.binding_is_live(&skin, &bound) {
            bound
        } else {
            self.bind_bones(owner, &skin)
        };
        let overrides = match self.world.mesh_mut(owner) {
            Some(mut mesh) => {
                mesh.skeleton.bones = bones.clone();
                std::mem::take(&mut mesh.skeleton.overrides)
            }
            None => return,
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
    }

    /// Whether `bones` is a complete binding of `skin` to live entities.
    fn binding_is_live(&self, skin: &SkinData, bones: &[u32]) -> bool {
        bones.len() == skin.local_bind.len() && bones.iter().all(|&b| self.world.contains(b))
    }

    /// Walk the joint slots in order (parents first), finding each bone by name
    /// under its parent or spawning it at rest. Returns the slot → entity binding.
    fn bind_bones(&mut self, owner: u32, skin: &SkinData) -> Vec<u32> {
        let mut bones: Vec<u32> = Vec::with_capacity(skin.local_bind.len());
        for slot in 0..skin.local_bind.len() {
            let parent = match skin.parents.get(slot).copied().flatten() {
                Some(p) if p < slot => bones[p],
                _ => owner,
            };
            let name = skin.name(slot);
            let existing =
                self.world.children(parent).into_iter().find(|&c| {
                    !bones.contains(&c) && self.world.name(c).is_some_and(|n| *n == name)
                });
            let bone =
                existing.unwrap_or_else(|| self.spawn_bone(parent, name, rest_local(skin, slot)));
            bones.push(bone);
        }
        bones
    }

    fn spawn_bone(&mut self, parent: u32, name: String, rest: TransformComponent) -> u32 {
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
