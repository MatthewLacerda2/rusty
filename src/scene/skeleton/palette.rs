//! The palette build (#453): skinning reads the bone GameObjects.
//!
//! Runs once per frame after every pose writer — the Animator, `LateUpdate`
//! scripts, physics — and before render, so whatever moved a bone last is what the
//! mesh shows. CPU-only, so the headless path poses exactly like the windowed one.

use glam::Mat4;

use crate::asset::SkinData;
use crate::scene::Scene;

impl Scene {
    /// Rebuild every skinned mesh's `pose_palette` from its bones' current
    /// transforms: `bone_in_mesh_space * inverse_bind` per joint, the same
    /// convention as the bind palette. A mesh whose skeleton is not bound keeps
    /// rendering its bind pose.
    pub fn build_skin_palettes(&mut self) {
        for owner in self.world.ids_with_mesh() {
            let Some(palette) = self.skin_palette(owner) else {
                continue;
            };
            if let Some(mut mesh) = self.world.mesh_mut(owner) {
                mesh.pose_palette = palette;
            }
        }
    }

    /// One mesh's palette. Each bone's mesh-space matrix is composed down the
    /// skeleton (parents first, so every parent is already resolved); a bone a
    /// script re-parented elsewhere falls back to its live world matrix, and a
    /// destroyed (missing) bone rides with its parent ([`missing_in_mesh`]).
    fn skin_palette(&self, owner: u32) -> Option<Vec<Mat4>> {
        let mesh = self.world.mesh(owner)?;
        let skin = mesh.skin.as_ref()?;
        let bones = &mesh.skeleton.bones;
        if bones.is_empty() || bones.len() != skin.inverse_bind.len() {
            return None;
        }
        let owner_inverse = self.world.local_to_world(owner).inverse();
        let mut in_mesh: Vec<Mat4> = Vec::with_capacity(bones.len());
        for (slot, &bone) in bones.iter().enumerate() {
            let parent_slot = skin.parents.get(slot).copied().flatten();
            let Some((bone, (local, parent))) =
                bone.and_then(|b| Some((b, self.world.local_and_parent(b)?)))
            else {
                in_mesh.push(missing_in_mesh(skin, slot, parent_slot, &in_mesh)?);
                continue;
            };
            let m = match (parent, parent_slot) {
                (Some(p), _) if p == owner => local,
                (Some(p), Some(ps)) if ps < slot && bones[ps] == Some(p) => in_mesh[ps] * local,
                _ => owner_inverse * self.world.local_to_world(bone),
            };
            in_mesh.push(m);
        }
        Some(
            in_mesh
                .iter()
                .zip(skin.inverse_bind.iter())
                .map(|(g, ib)| *g * *ib)
                .collect(),
        )
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
