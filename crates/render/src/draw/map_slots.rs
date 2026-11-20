//! The material's map slots (#202, #207, #1997, #2151, #2191, #2258): one table, [`MAP_SLOTS`],
//! whose rows say which asset field each group-2 texture comes from, at which
//! binding, holding what, and which record flag goes off while it has no texture.
//! `materials` builds the bind group from it; a new map is one [`Map`] and one row
//! (PR #2151's *Seam*).

use crate::gpu::material_cache::{MATERIAL_MAPS, MATERIAL_TEXTURES};
use crate::gpu::textures::TextureKind;
use crate::gpu::uniforms::LevelMapsUniform;
use crate::MaterialData;
use rusty_asset::MapRef;
use rusty_components::MaterialAsset;
use rusty_shadergen::textures;

/// A material map, by its row of [`MAP_SLOTS`] (`map as usize`, which a const
/// assert holds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Map {
    Albedo,
    Metallic,
    Roughness,
    Normal,
    Emissive,
    Detail,
    Occlusion,
    Specular,
    Height,
    DetailMask,
}

/// One group-2 material map: everything the renderer knows about it, in one row.
pub(crate) struct MapSlot {
    pub map: Map,
    /// Its texture's group-2 binding; its sampler's is [`map_sampler_binding`].
    pub binding: u32,
    /// What it holds, which decides its view (sRGB for `Colour`, raw otherwise,
    /// glTF 2.0, #647), how an uncompressed upload's mips average (#1322) and which
    /// compressed formats it can read (#1574).
    pub kind: TextureKind,
    /// The asset field naming its texture.
    pub path: fn(&MaterialAsset) -> Option<&MapRef>,
    /// The record flag that goes to 0 while the map has no resident texture
    /// (missing, mistyped, loading, an empty path), so the surface draws as if
    /// unmapped rather than over the checker its slot binds meanwhile: set for
    /// every map that darkens or scales a term (the detail map, #1997; occlusion
    /// and specular level, #2151). `None` keeps the flag, the checker showing.
    pub off_while_missing: Option<fn(&mut MaterialData) -> &mut u32>,
}

/// Every material map, in [`Map`] order. Albedo, metallic, roughness, normal and
/// emissive at 0 and 2–5 (#202, #207); the detail map at 15 (#1997), the last
/// texture binding below the samplers' offset, then occlusion at 14 and specular
/// level at 13 (#2151), height at 12 (#2191) and the detail mask at 11 (#2258),
/// counting down, so the extra shader slots keep the bindings after the param
/// uniform (7, …) and a new map takes the next one down (10).
pub(crate) const MAP_SLOTS: [MapSlot; MATERIAL_MAPS] = [
    MapSlot {
        map: Map::Albedo,
        binding: 0,
        kind: TextureKind::Colour,
        path: |m| m.base_color_map.as_ref(),
        off_while_missing: None,
    },
    MapSlot {
        map: Map::Metallic,
        binding: 2,
        kind: TextureKind::Mask,
        path: |m| m.metallic_map.as_ref(),
        off_while_missing: None,
    },
    MapSlot {
        map: Map::Roughness,
        binding: 3,
        kind: TextureKind::Mask,
        path: |m| m.roughness_map.as_ref(),
        off_while_missing: None,
    },
    MapSlot {
        map: Map::Normal,
        binding: 4,
        kind: TextureKind::Normal,
        path: |m| m.normal_map.as_ref(),
        off_while_missing: None,
    },
    MapSlot {
        map: Map::Emissive,
        binding: 5,
        kind: TextureKind::Colour,
        path: |m| m.emissive_map.as_ref(),
        off_while_missing: None,
    },
    MapSlot {
        map: Map::Detail,
        binding: 15,
        kind: TextureKind::Data,
        path: |m| m.detail.as_ref().and_then(|d| d.map.as_ref()),
        off_while_missing: Some(detail_flag),
    },
    MapSlot {
        map: Map::Occlusion,
        binding: 14,
        kind: TextureKind::Mask,
        path: |m| m.occlusion_map.as_ref(),
        off_while_missing: Some(occlusion_flag),
    },
    MapSlot {
        map: Map::Specular,
        binding: 13,
        kind: TextureKind::Mask,
        path: |m| m.specular_map.as_ref(),
        off_while_missing: Some(specular_flag),
    },
    MapSlot {
        map: Map::Height,
        binding: 12,
        kind: TextureKind::Mask,
        path: |m| m.height_map.as_ref(),
        off_while_missing: Some(height_flag),
    },
    MapSlot {
        map: Map::DetailMask,
        binding: 11,
        kind: TextureKind::Mask,
        // Bound only beside a detail map: a mask alone draws nothing.
        path: |m| {
            let d = m.detail.as_ref().filter(|d| d.map.is_some())?;
            d.mask.as_ref()
        },
        off_while_missing: Some(detail_mask_flag),
    },
];

fn detail_flag(record: &mut MaterialData) -> &mut u32 {
    &mut record.detail.use_detail_map
}

fn occlusion_flag(record: &mut MaterialData) -> &mut u32 {
    &mut record.level_maps.use_occlusion_map
}

fn specular_flag(record: &mut MaterialData) -> &mut u32 {
    &mut record.level_maps.use_specular_map
}

fn height_flag(record: &mut MaterialData) -> &mut u32 {
    &mut record.level_maps.use_height_map
}

fn detail_mask_flag(record: &mut MaterialData) -> &mut u32 {
    &mut record.detail.use_detail_mask
}

// Each row sits at its `Map`'s index, and no two textures (maps and extra shader
// slots) share a binding, all below the samplers' offset.
const _: () = {
    let bindings = all_bindings();
    let mut i = 0;
    while i < MATERIAL_TEXTURES {
        if i < MATERIAL_MAPS {
            assert!(MAP_SLOTS[i].map as usize == i, "a row out of `Map` order");
        }
        assert!(bindings[i] < textures::SAMPLER_OFFSET);
        let mut j = i + 1;
        while j < MATERIAL_TEXTURES {
            assert!(
                bindings[i] != bindings[j],
                "two group-2 textures share a binding"
            );
            j += 1;
        }
        i += 1;
    }
};

/// Every group-2 texture binding, in [`material_texture_paths`] order: the maps'
/// rows, then each extra shader slot's (#400).
pub(crate) const fn all_bindings() -> [u32; MATERIAL_TEXTURES] {
    let mut out = [0; MATERIAL_TEXTURES];
    let mut i = 0;
    while i < MATERIAL_TEXTURES {
        out[i] = match i < MATERIAL_MAPS {
            true => MAP_SLOTS[i].binding,
            false => textures::SLOTS[i - MATERIAL_MAPS].binding,
        };
        i += 1;
    }
    out
}

/// The binding of the sampler paired with the material texture at `binding`:
/// albedo's `s_diffuse` at 1, every other at `textures::sampler_binding` (#1323).
pub(crate) fn map_sampler_binding(binding: u32) -> u32 {
    match binding {
        0 => 1,
        b => textures::sampler_binding(b),
    }
}

/// What the group-2 texture at `index` (in [`material_texture_paths`] order) holds:
/// its row's kind, and data for an extra shader texture slot.
pub(crate) fn map_kind(index: usize) -> TextureKind {
    MAP_SLOTS.get(index).map_or(TextureKind::Data, |s| s.kind)
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

/// Every texture path `material`'s group 2 binds, in [`MapSignature`] order: the
/// maps, in [`MAP_SLOTS`] order, then the texture each extra shader slot names
/// (#400). Also the list of textures to upload before the group is built.
///
/// [`MapSignature`]: crate::gpu::material_cache::MapSignature
pub(crate) fn material_texture_paths(
    material: Option<&MaterialAsset>,
) -> [Option<MapRef>; MATERIAL_TEXTURES] {
    let Some(m) = material else {
        return Default::default();
    };
    let maps = MAP_SLOTS.iter().map(|s| (s.path)(m));
    let slots = textures::SLOTS
        .iter()
        .map(|s| m.shader_textures.get(s.name));
    let mut paths = maps.chain(slots);
    std::array::from_fn(|_| paths.next().flatten().cloned())
}

/// The record's occlusion, specular-level (#2151) and height (#2191) terms: each
/// map's flag, on where a path is named (the specular one raised to 2 for a
/// one-channel upload, and each turned off while its texture is not resident, by
/// `with_map_channels`; the height one only where it applies, else warned of once),
/// the occlusion strength and the height's depth, all off without a material.
pub(crate) fn level_maps_uniform(material: Option<&MaterialAsset>) -> LevelMapsUniform {
    let Some(m) = material else {
        return LevelMapsUniform {
            occlusion_strength: 1.0,
            ..LevelMapsUniform::default()
        };
    };
    LevelMapsUniform {
        use_occlusion_map: u32::from(m.occlusion_map.is_some()),
        occlusion_strength: m.occlusion_amount(),
        use_specular_map: u32::from(m.specular_map.is_some()),
        use_height_map: u32::from(height_applies(m)),
        height_scale: m.height_depth(),
        _pad: [0; 3],
    }
}

/// Whether `m`'s height map displaces it ([`MaterialAsset::height_applies`]); a map
/// it ignores (`Cutout`, `Transparent`, `Triplanar`) is said once in the console.
fn height_applies(m: &MaterialAsset) -> bool {
    let applies = m.height_applies();
    if let (false, Some(path)) = (applies, m.height_map.as_ref()) {
        let why = match m.uv_mapping {
            rusty_components::UvMapping::Triplanar => "under Triplanar".to_string(),
            _ => format!("on a {:?} material", m.render_mode),
        };
        let key = format!("height-map-ignored:{path}:{why}");
        rusty_scene::rehydrate::warn_once_of(&key, || {
            format!("height map '{path}' is ignored {why}: only an Opaque UV0 or Planar material is displaced")
        });
    }
    applies
}

/// The resolved material map paths (albedo, metallic, roughness, normal, emissive,
/// then the extra shader texture slots, #400) the forward shader samples, of every
/// entity `admit` passes, each with what its slot holds (#1322): which render
/// textures a frame's materials name (`render_texture`).
pub(crate) fn material_map_paths(
    scene: &rusty_scene::Scene,
    admit: impl Fn(u32) -> bool,
) -> Vec<(MapRef, TextureKind)> {
    scene
        .world
        .ids_with_material()
        .into_iter()
        .filter(|&id| admit(id))
        .flat_map(|id| scene.material_assets_of(id))
        .flat_map(|m| material_maps(Some(m)))
        .collect()
}

#[cfg(test)]
#[path = "map_slots_tests.rs"]
mod tests;
