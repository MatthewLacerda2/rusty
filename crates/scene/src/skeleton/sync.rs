use std::collections::BTreeSet;

use glam::Mat4;

use super::bone_refs::BoneRef;
use crate::Scene;
use rusty_asset::SkinData;
use rusty_components::mesh::BoneBody;
use rusty_components::TransformComponent;

impl Scene {
    pub fn sync_skeletons(&mut self) {
        let mut refs = Vec::new();
        for owner in self.world.ids_with_mesh() {
            refs.extend(self.sync_skeleton_past(owner, &BTreeSet::new()));
        }
        self.bind_bone_refs(refs);
    }

    pub fn sync_skeleton(&mut self, owner: u32) {
        let refs = self.sync_skeleton_past(owner, &BTreeSet::new());
        self.bind_bone_refs(refs);
    }

    pub(super) fn sync_skeleton_past(
        &mut self,
        owner: u32,
        parked: &BTreeSet<u32>,
    ) -> Vec<BoneRef> {
        let Some((skin, bound, pending)) = self.world.mesh(owner).and_then(|m| {
            let pending = !m.skeleton.overrides.is_empty() || !m.skeleton.bodies.is_empty();
            Some((m.skin.clone()?, m.skeleton.bones.clone(), pending))
        }) else {
            return Vec::new();
        };
        let rebound = (!self.binding_is_live(&skin, &bound))
            .then(|| self.bind_bones(owner, &skin, parked))
            .filter(|bones| *bones != bound);
        if rebound.is_none() && !pending {
            return Vec::new();
        }
        let (overrides, bodies) = match self.world.mesh_mut(owner) {
            Some(mut mesh) => {
                if let Some(bones) = rebound {
                    mesh.skeleton.bones = bones;
                }
                (
                    std::mem::take(&mut mesh.skeleton.overrides),
                    std::mem::take(&mut mesh.skeleton.bodies),
                )
            }
            None => return Vec::new(),
        };
        for (name, transform) in overrides {
            match self.live_bone(owner, &name) {
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

    fn restore_bone_body(&mut self, owner: u32, name: &str, body: BoneBody) -> Vec<BoneRef> {
        let Some(bone) = self.live_bone(owner, name) else {
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
        let mut landed = None;
        if let Some(j) = joint.as_mut().filter(|_| !body.connected_bone.is_empty()) {
            let (to, moved) = self.resolve_chain(owner, &body.connected_bone);
            j.connected_body = Some(to);
            landed = moved.map(|what| (to, what));
        }
        self.world.set_rigidbody(bone, body.rigidbody);
        self.world.set_joint(bone, joint);
        if let Some((to, what)) = landed {
            let lost = body.connected_bone.first().map_or("", String::as_str);
            log::warn!(
                "[Asset] entity {owner}: bone '{name}''s joint was to bone '{lost}', which \
                 the skeleton no longer has; it now joins {what}, {}.",
                self.joint_effect(bone, to)
            );
        }
        refs
    }

    fn joint_effect(&self, bone: u32, to: u32) -> String {
        let own = self.world.body_carrier(bone);
        match self.world.body_carrier(to) {
            Some(carrier) if Some(carrier) != own => {
                format!("constraining it to entity {carrier}'s body")
            }
            _ => "so the joint is inert".to_string(),
        }
    }

    fn binding_is_live(&self, skin: &SkinData, bones: &[Option<u32>]) -> bool {
        let live = |b: &u32| self.world.contains(*b);
        bones.len() == skin.local_bind.len() && bones.iter().flatten().all(live)
    }

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
