use glam::Mat4;

use crate::Scene;
use rusty_asset::SkinData;

impl Scene {
    pub fn build_skin_palettes(&mut self) {
        let mut palette = Vec::new();
        for owner in self.skinned_ids() {
            if self.skin_palette(owner, &mut palette) {
                self.world.set_pose_palette(owner, &palette);
            }
        }
    }

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
                // A bone a script re-parented elsewhere skins by its live world matrix.
                _ => owner_inverse * self.world.local_to_world(bone),
            };
            out.push(m);
        }
        // Inverse binds fold in only now: until here each child read its parent's bare matrix
        // from `out`.
        for (g, ib) in out.iter_mut().zip(skin.inverse_bind.iter()) {
            *g *= *ib;
        }
        true
    }
}

/// A destroyed slot rides with its parent joint (no head floating at the bind pose
/// after a decapitation); only a skeleton root falls back to its bind pose.
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
