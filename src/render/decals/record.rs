//! One decal as the forward shader reads it (#638): the box, its frame, what it
//! changes and by how much, and the atlas layers it samples. Pure CPU data, built
//! from the sim's [`Decal`] and the material it names.

use glam::Vec3;

use crate::asset::MapRef;
use crate::components::{DecalBlend, MaterialAsset};
use crate::render::gpu::textures::TextureKind;
use crate::scene::decal::{Decal, DecalPose};

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
pub(crate) type DecalMaps = [Option<MapRef>; 4];

/// What each of [`DecalMaps`]' slots holds, which decides how its mips average
/// (#1364): the albedo is colour, the rest data.
const MAP_KINDS: [TextureKind; 4] = [
    TextureKind::Colour,
    TextureKind::Data,
    TextureKind::Data,
    TextureKind::Data,
];

/// The maps of `maps` that are set, each with the kind its slot samples it as.
pub(crate) fn kinded(maps: &DecalMaps) -> impl Iterator<Item = (&MapRef, TextureKind)> {
    maps.iter()
        .zip(MAP_KINDS)
        .filter_map(|(map, kind)| Some((map.as_ref()?, kind)))
}

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

/// `decal`'s GPU record at `pose` (#639), dressed by `material` (the library
/// material it names, if any) and sampling the atlas `layers` of its [`maps`]. The
/// pose's fade scales the coverage (`color.a`), which scales every blend weight.
pub(crate) fn record(
    decal: &Decal,
    pose: &DecalPose,
    material: Option<&MaterialAsset>,
    layers: [u32; 4],
) -> GpuDecal {
    let blend = material.map_or(DecalBlend::ALBEDO_ONLY, |m| m.decal.clamped());
    let [r, g, b, a] = decal.color;
    let tint = [r, g, b, a * pose.opacity];
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
    let axis = |v: Vec3, w: f32| {
        let along = pose.model.transform_vector3(v).normalize_or_zero();
        along.extend(w).to_array()
    };
    let fade_start = blend.angle_fade;
    let fade_end = (fade_start + ANGLE_FADE_SPAN).min(90.0);
    GpuDecal {
        world_to_decal: pose.model.inverse().to_cols_array(),
        right: axis(Vec3::X, blend.albedo),
        up: axis(Vec3::Y, blend.normal),
        forward: axis(Vec3::Z, fade_start.to_radians().cos()),
        color,
        surface: [metallic, roughness, blend.metallic, blend.roughness],
        fade: [blend.occlusion, fade_end.to_radians().cos(), 0.0, 0.0],
        layers,
    }
}

/// The sphere that bounds the box at `pose`, for cluster binning: centre and radius.
pub(crate) fn bounds(pose: &DecalPose) -> (Vec3, f32) {
    (pose.centre(), pose.radius())
}

#[cfg(test)]
#[path = "record_tests.rs"]
mod tests;

#[cfg(test)]
mod kind_tests {
    use super::*;

    #[test]
    fn the_albedo_slot_is_colour_and_the_others_data() {
        let [a, n, r] = ["a.png", "n.png", "r.png"].map(MapRef::from);
        let maps = [Some(a.clone()), Some(n.clone()), None, Some(r.clone())];
        let got: Vec<_> = kinded(&maps).collect();
        let (colour, data) = (TextureKind::Colour, TextureKind::Data);
        assert_eq!(got, [(&a, colour), (&n, data), (&r, data)]);
    }
}
