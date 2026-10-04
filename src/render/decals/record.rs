//! One decal as the forward shader reads it (#638): the box, its frame, what it
//! changes and by how much, and the atlas layers it samples. Pure CPU data, built
//! from the sim's [`Decal`] and the material it names.

use glam::Vec3;

use crate::components::{DecalBlend, MaterialAsset};
use crate::scene::decal::Decal;

/// `GpuDecal::layers` for a map the decal does not have.
pub(crate) const NO_LAYER: u32 = u32::MAX;

/// How far past `angle_fade` a surface turns before the decal is gone, in degrees.
const ANGLE_FADE_SPAN: f32 = 15.0;

/// One decal, mirroring `Decal` in `common.wgsl` byte-for-byte.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuDecal {
    /// World → box space, where the box is the unit cube `[-0.5, 0.5]³`.
    pub world_to_decal: [f32; 16],
    /// The box's world-space right axis (texture `u`); `w` the albedo weight.
    pub right: [f32; 4],
    /// The box's world-space up axis (texture `-v`); `w` the normal weight.
    pub up: [f32; 4],
    /// The axis out of the surface (against the projection); `w` the cosine of the
    /// angle where the angle fade starts.
    pub forward: [f32; 4],
    /// Albedo tint (rgb) and coverage (a).
    pub color: [f32; 4],
    /// Metallic, roughness, then their weights.
    pub surface: [f32; 4],
    /// `x` the occlusion it adds, `y` the cosine where the angle fade ends.
    pub fade: [f32; 4],
    /// Atlas layers of the albedo, normal, metallic and roughness maps, or
    /// [`NO_LAYER`].
    pub layers: [u32; 4],
}

/// The maps one decal samples, by path: albedo, normal, metallic, roughness.
pub(crate) type DecalMaps = [Option<String>; 4];

/// The maps `decal` samples: its material's when it names one, else its own
/// albedo `texture`.
pub(crate) fn maps(decal: &Decal, material: Option<&MaterialAsset>) -> DecalMaps {
    match material {
        Some(m) => [
            m.base_color_map.clone().or_else(|| decal.texture.clone()),
            m.normal_map.clone(),
            m.metallic_map.clone(),
            m.roughness_map.clone(),
        ],
        None => [decal.texture.clone(), None, None, None],
    }
}

/// `decal`'s GPU record, dressed by `material` (the library material it names, if
/// any) and sampling the atlas `layers` of its [`maps`].
pub(crate) fn record(
    decal: &Decal,
    material: Option<&MaterialAsset>,
    layers: [u32; 4],
) -> GpuDecal {
    let blend = material.map_or(DecalBlend::ALBEDO_ONLY, |m| m.decal.clamped());
    let tint = decal.color;
    let (color, metallic, roughness) = match material {
        Some(m) => {
            let c = m.base_color;
            let rgb = [c[0] * tint[0], c[1] * tint[1], c[2] * tint[2]];
            (
                [rgb[0], rgb[1], rgb[2], m.alpha * tint[3]],
                m.metallic,
                m.roughness,
            )
        }
        None => (tint, 0.0, 0.5),
    };
    let axis = |v: Vec3, w: f32| (decal.rotation * v).extend(w).to_array();
    let fade_start = blend.angle_fade;
    let fade_end = (fade_start + ANGLE_FADE_SPAN).min(90.0);
    GpuDecal {
        world_to_decal: decal.model_matrix().inverse().to_cols_array(),
        right: axis(Vec3::X, blend.albedo),
        up: axis(Vec3::Y, blend.normal),
        forward: axis(Vec3::Z, fade_start.to_radians().cos()),
        color,
        surface: [metallic, roughness, blend.metallic, blend.roughness],
        fade: [blend.occlusion, fade_end.to_radians().cos(), 0.0, 0.0],
        layers,
    }
}

/// The sphere that bounds `decal`'s box, for cluster binning: centre and radius.
pub(crate) fn bounds(decal: &Decal) -> (Vec3, f32) {
    (decal.position, decal.size.length() * 0.5)
}

#[cfg(test)]
#[path = "record_tests.rs"]
mod tests;
