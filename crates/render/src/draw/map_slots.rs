//! The material's map slots (#202, #207, #1997, #2151): which asset field each
//! group-2 texture comes from, at which binding, holding what, and the record's
//! flags for the occlusion and specular-level maps. `materials` builds the bind
//! group from them; a new map is one entry in each list here (PR #2151's *Seam*).

use crate::gpu::material_cache::{MATERIAL_MAPS, MATERIAL_TEXTURES};
use crate::gpu::textures::TextureKind;
use crate::gpu::uniforms::LevelMapsUniform;
use rusty_asset::MapRef;
use rusty_components::MaterialAsset;
use rusty_shadergen::textures;

/// Each map's group-2 binding, in [`material_texture_paths`] order: albedo,
/// metallic, roughness, normal and emissive at 0 and 2–5 (#202, #207), the detail
/// map at 15 (#1997), the last texture binding below the samplers' offset, then
/// occlusion at 14 and specular level at 13 (#2151), counting down from it, so the
/// extra shader slots keep the bindings after the param uniform (7, …) and a new map
/// takes the next one down (12).
pub(crate) const MAP_BINDINGS: [u32; MATERIAL_MAPS] = [0, 2, 3, 4, 5, 15, 14, 13];

/// The binding of the sampler paired with the material texture at `binding`:
/// albedo's `s_diffuse` at 1, every other at `textures::sampler_binding` (#1323).
pub(super) fn map_sampler_binding(binding: u32) -> u32 {
    match binding {
        0 => 1,
        b => textures::sampler_binding(b),
    }
}

/// Whether the map at `index` (in [`material_texture_paths`] order) holds colour —
/// albedo or emissive, sampled through the sRGB-decoding view. Every other map and
/// shader texture slot holds data and samples its texels raw (glTF 2.0, #647).
pub(crate) fn is_colour_map(index: usize) -> bool {
    matches!(index, 0 | 4)
}

/// Every texture `material` samples (as [`material_texture_paths`]), each with what
/// its slot holds, which decides how its mips average (#1322): what to upload
/// before its bind group is built.
pub(crate) fn material_maps(
    material: Option<&MaterialAsset>,
) -> impl Iterator<Item = (MapRef, TextureKind)> {
    material_texture_paths(material)
        .into_iter()
        .enumerate()
        .filter_map(|(i, path)| Some((path?, map_kind(i))))
}

/// What the map at `index` holds, which decides how an uncompressed upload's mips
/// average (#1322) and which compressed formats it can read (#1574): metallic,
/// roughness, occlusion and specular level (#2151) are masks, the normal map a normal
/// map, the detail map (#1997) and the shader texture slots data.
pub(super) fn map_kind(index: usize) -> TextureKind {
    match index {
        1 | 2 | 6 | 7 => TextureKind::Mask,
        3 => TextureKind::Normal,
        i if is_colour_map(i) => TextureKind::Colour,
        _ => TextureKind::Data,
    }
}

/// Every texture path `material`'s group 2 binds, in [`MapSignature`] order: the
/// eight maps (the detail map, occlusion and specular level last, #1997, #2151), then the texture each extra shader slot
/// names (#400). Also the list
/// of textures to upload before the group is built.
///
/// [`MapSignature`]: crate::gpu::material_cache::MapSignature
pub(crate) fn material_texture_paths(
    material: Option<&MaterialAsset>,
) -> [Option<MapRef>; MATERIAL_TEXTURES] {
    let Some(m) = material else {
        return Default::default();
    };
    let maps = [
        &m.base_color_map,
        &m.metallic_map,
        &m.roughness_map,
        &m.normal_map,
        &m.emissive_map,
        &m.detail.as_ref().and_then(|d| d.map.clone()),
        &m.occlusion_map,
        &m.specular_map,
    ];
    let slots = textures::SLOTS
        .iter()
        .map(|s| m.shader_textures.get(s.name));
    let mut paths = maps.into_iter().map(Option::as_ref).chain(slots);
    std::array::from_fn(|_| paths.next().flatten().cloned())
}

/// The record's occlusion and specular-level terms (#2151): each map's flag (the
/// specular one raised to 2 for a one-channel upload by `with_map_channels`) and
/// the occlusion strength, all off without a material.
pub(crate) fn level_maps_uniform(material: Option<&MaterialAsset>) -> LevelMapsUniform {
    let Some(m) = material else {
        return LevelMapsUniform {
            occlusion_strength: 1.0,
            ..LevelMapsUniform::default()
        };
    };
    LevelMapsUniform {
        use_occlusion_map: u32::from(m.occlusion_map.is_some()),
        occlusion_strength: occlusion_strength(m.occlusion_strength),
        use_specular_map: u32::from(m.specular_map.is_some()),
        _pad: 0,
    }
}

/// An occlusion map's strength as the shader reads it (#2151): in `[0, 1]`, glTF's
/// range; a hand-written NaN applies the map whole, as the default does.
pub(crate) fn occlusion_strength(strength: f32) -> f32 {
    match strength.is_nan() {
        true => 1.0,
        false => strength.clamp(0.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_new_maps_are_masks_counting_down_from_the_detail_binding() {
        let m = MaterialAsset {
            occlusion_map: Some("orm.png".into()),
            specular_map: Some("level.png".into()),
            ..MaterialAsset::default()
        };
        let paths = material_texture_paths(Some(&m));
        assert_eq!(paths[6], m.occlusion_map);
        assert_eq!(paths[7], m.specular_map);
        assert_eq!((MAP_BINDINGS[6], MAP_BINDINGS[7]), (14, 13));
        assert!(matches!(map_kind(6), TextureKind::Mask));
        assert!(matches!(map_kind(7), TextureKind::Mask));
        assert!(!is_colour_map(6) && !is_colour_map(7), "data, sampled raw");
    }

    #[test]
    fn the_level_maps_flag_what_is_named_and_hold_the_strength_in_range() {
        let off = level_maps_uniform(None);
        assert_eq!((off.use_occlusion_map, off.occlusion_strength), (0, 1.0));
        let m = MaterialAsset {
            occlusion_map: Some("ao.png".into()),
            occlusion_strength: 0.25,
            ..MaterialAsset::default()
        };
        let on = level_maps_uniform(Some(&m));
        assert_eq!(
            (
                on.use_occlusion_map,
                on.occlusion_strength,
                on.use_specular_map
            ),
            (1, 0.25, 0)
        );
        assert_eq!(
            occlusion_strength(f32::NAN),
            1.0,
            "a NaN applies the map whole"
        );
        assert_eq!(
            (occlusion_strength(-2.0), occlusion_strength(9.0)),
            (0.0, 1.0)
        );
    }
}
