use glam::Vec3;

use crate::gpu::textures::TextureKind;
use rusty_asset::MapRef;
use rusty_components::{DecalBlend, MaterialAsset};
use rusty_scene::decal::{Decal, DecalPose};

/// A `layers` entry for an absent map; mirrors `NO_LAYER` in `common.wgsl`.
pub(crate) const NO_LAYER: u32 = u32::MAX;

/// Degrees past `angle_fade` at which the decal is gone (capped at 90 in `record`).
const ANGLE_FADE_SPAN: f32 = 15.0;

/// Mirrors `Decal` in `common.wgsl`; `gpu::uniform_layout_tests` holds the layout.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuDecal {
    pub world_to_decal: [f32; 16],
    pub right: [f32; 4],
    pub up: [f32; 4],
    pub forward: [f32; 4],
    pub color: [f32; 4],
    pub surface: [f32; 4],
    pub fade: [f32; 4],
    pub layers: [u32; 4],
}

pub(crate) type DecalMaps = [Option<MapRef>; 4];

const MAP_KINDS: [TextureKind; 4] = [
    TextureKind::Colour,
    TextureKind::Data,
    TextureKind::Data,
    TextureKind::Data,
];

pub(crate) fn kinded(maps: &DecalMaps) -> impl Iterator<Item = (&MapRef, TextureKind)> {
    maps.iter()
        .zip(MAP_KINDS)
        .filter_map(|(map, kind)| Some((map.as_ref()?, kind)))
}

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
