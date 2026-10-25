//! src/asset/scale_factor.rs — a model's import *Scale Factor* (#1398).
//!
//! Unity's model-import Scale Factor: one uniform scale the sidecar stores and the
//! importer applies to the model's data, so everything downstream (colliders, nav
//! bakes, the lightmap unwrap, the manifest's footprint, the bones) sees the scaled
//! model and never the authored one.
//!
//! Scaling the model by `s` is `S · M · S⁻¹` for every transform in it: a uniform
//! scale commutes with any linear part, so each matrix keeps its rotation and scale
//! and only its translation is multiplied by `s`. That holds the bind palette
//! (`bind_global · inverse_bind`) at `S · palette · S⁻¹`, exactly what the scaled
//! vertices need; normals and tangents are directions and stay as they are.

use glam::Mat4;

use super::anim_data::AnimationClip;
use super::mesh_data::{ImportedAsset, SkinData, SubMesh};

/// The smallest Scale Factor the sidecar keeps: a zero or negative one would
/// collapse or mirror the model.
pub const MIN_SCALE: f32 = 1e-4;

/// `requested` when it is a usable Scale Factor, else `current` for a NaN or
/// infinite one, and [`MIN_SCALE`] for one at or below it: a bad input changes
/// nothing, as the authoring setters' rule is (#939).
pub fn sanitize(requested: f32, current: f32) -> f32 {
    if requested.is_finite() {
        requested.max(MIN_SCALE)
    } else {
        current
    }
}

/// Scale every sub-mesh of `asset` by `s`. A factor of 1 leaves it untouched.
pub fn apply(asset: &mut ImportedAsset, s: f32) {
    if s == 1.0 {
        return;
    }
    for sub in &mut asset.sub_meshes {
        scale_sub_mesh(sub, s);
    }
}

fn scale_sub_mesh(sub: &mut SubMesh, s: f32) {
    for v in &mut sub.vertices {
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

/// `S · m · S⁻¹` for a uniform `S`: `m` with its translation column scaled.
fn scale_translation(mut m: Mat4, s: f32) -> Mat4 {
    m.w_axis = (m.w_axis.truncate() * s).extend(m.w_axis.w);
    m
}

#[cfg(test)]
#[path = "scale_factor_tests.rs"]
mod tests;
