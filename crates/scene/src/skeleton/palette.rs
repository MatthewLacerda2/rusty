//! The palette build (#453): skinning reads the bone GameObjects.
//!
//! Runs once per frame after every pose writer — the Animator, `LateUpdate`
//! scripts, physics — and before render, so whatever moved a bone last is what the
//! mesh shows. CPU-only, so the headless path poses exactly like the windowed one.

use glam::Mat4;

use crate::Scene;
use rusty_asset::SkinData;

impl Scene {
    /// Rebuild every skinned mesh's `pose_palette` from its bones' current
    /// transforms: `bone_in_mesh_space * inverse_bind` per joint, the same
    /// convention as the bind palette. A mesh whose skeleton is not bound keeps
    /// rendering its bind pose. Visits the skinned entities only
    /// ([`Scene::skinned_ids`]), never the scene's static meshes, and composes
    /// every rig in one scratch, copied into the palette each mesh already holds.
    pub fn build_skin_palettes(&mut self) {
        let mut palette = Vec::new();
        for owner in self.skinned_ids() {
            if self.skin_palette(owner, &mut palette) {
                self.world.set_pose_palette(owner, &palette);
            }
        }
    }

    /// One mesh's palette, into `out` (cleared first); `false` when the mesh has
    /// no bound skeleton. Each bone's mesh-space matrix is composed down the
    /// skeleton (parents first, so every parent is already resolved); a bone a
    /// script re-parented elsewhere falls back to its live world matrix, and a
    /// destroyed (missing) bone rides with its parent ([`missing_in_mesh`]).
    fn skin_palette(&self, owner: u32, out: &mut Vec<Mat4>) -> bool {
        out.clear();
        let Some(mesh) = self.world.mesh(owner) else {
            return false;
        };
        let Some(skin) = mesh.skin.as_ref() else {
            return false;
        };
        let bones = &mesh.skeleton.bones;
        if bones.is_empty() || bones.len() != skin.inverse_bind.len() {
            return false;
        }
        let owner_inverse = self.world.local_to_world(owner).inverse();
        for (slot, &bone) in bones.iter().enumerate() {
            let parent_slot = skin.parents.get(slot).copied().flatten();
            let Some((bone, (local, parent))) =
                bone.and_then(|b| Some((b, self.world.local_and_parent(b)?)))
            else {
                let Some(m) = missing_in_mesh(skin, slot, parent_slot, out) else {
                    return false;
                };
                out.push(m);
                continue;
            };
            let m = match (parent, parent_slot) {
                (Some(p), _) if p == owner => local,
                (Some(p), Some(ps)) if ps < slot && bones[ps] == Some(p) => out[ps] * local,
                _ => owner_inverse * self.world.local_to_world(bone),
            };
            out.push(m);
        }
        // Every mesh-space matrix is composed; only now fold in the inverse binds,
        // since a child above read its parent's mesh-space matrix from `out`.
        for (g, ib) in out.iter_mut().zip(skin.inverse_bind.iter()) {
            *g *= *ib;
        }
        true
    }
}

/// A missing (destroyed, #1273) slot's mesh-space matrix: its bind local on top of
/// its parent joint's posed matrix, so it rides with the nearest surviving bone
/// above it (no head left floating at the bind pose after a decapitation); its
/// bind pose only for a skeleton root, when no parent is left to ride with.
fn missing_in_mesh(
    skin: &SkinData,
    slot: usize,
    parent_slot: Option<usize>,
    in_mesh: &[Mat4],
) -> Option<Mat4> {
    match parent_slot.filter(|&p| p < slot) {
        Some(p) => Some(in_mesh[p] * skin.local_bind.get(slot)?.matrix()),
        None => skin.bind_global.get(slot).copied(),
    }
}
