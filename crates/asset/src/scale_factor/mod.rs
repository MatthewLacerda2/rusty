//! One uniform scale applied to the model's data at import, so colliders, nav bakes, the lightmap
//! unwrap and bones all see the scaled model.
//!
//! Scaling by `s` is `S · M · S⁻¹` for every transform: only each matrix's translation is
//! multiplied; normals and tangents are directions and stay.

use glam::Mat4;

use super::anim_data::AnimationClip;
use super::mesh_data::{ImportedAsset, SkinData, SubMesh};

/// Smallest factor kept: zero or negative would collapse or mirror the model.
pub const MIN_SCALE: f32 = 1e-4;

/// `requested` if usable, else `current` for NaN/infinite, and [`MIN_SCALE`] at or below it.
pub fn sanitize(requested: f32, current: f32) -> f32 {
    if requested.is_finite() {
        requested.max(MIN_SCALE)
    } else {
        current
    }
}

pub fn apply(asset: &mut ImportedAsset, s: f32) {
    if s == 1.0 {
        return;
    }
    for sub in &mut asset.sub_meshes {
        scale_sub_mesh(sub, s);
    }
    // Node offsets and bone rest poses are in parent space: they scale too.
    for node in &mut asset.nodes {
        node.translation *= s;
        if let Some(bone) = &mut node.bone {
            bone.rest = scale_translation(bone.rest, s);
        }
    }
}

fn scale_sub_mesh(sub: &mut SubMesh, s: f32) {
    for v in sub.vertices.edit() {
        v.position = v.position.map(|c| c * s);
    }
    if let Some(skin) = &mut sub.skin {
        scale_skin(skin, s);
    }
    for clip in &mut sub.clips {
        scale_clip(clip, s);
    }
}

fn scale_skin(skin: &mut SkinData, s: f32) {
    for m in skin
        .inverse_bind
        .iter_mut()
        .chain(&mut skin.bind_global)
        .chain(std::iter::once(&mut skin.mesh_inverse))
    {
        *m = scale_translation(*m, s);
    }
    for local in &mut skin.local_bind {
        local.translation *= s;
    }
}

fn scale_clip(clip: &mut AnimationClip, s: f32) {
    for track in &mut clip.tracks {
        for t in &mut track.translation.values {
            *t *= s;
        }
    }
}

fn scale_translation(mut m: Mat4, s: f32) -> Mat4 {
    m.w_axis = (m.w_axis.truncate() * s).extend(m.w_axis.w);
    m
}

#[cfg(test)]
mod tests;
