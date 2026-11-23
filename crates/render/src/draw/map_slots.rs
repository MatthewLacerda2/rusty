use crate::gpu::material_cache::{MATERIAL_MAPS, MATERIAL_TEXTURES};
use crate::gpu::textures::TextureKind;
use crate::gpu::uniforms::LevelMapsUniform;
use crate::MaterialData;
use rusty_asset::MapRef;
use rusty_components::MaterialAsset;
use rusty_shadergen::textures;

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
    SpecularColor,
}

pub(crate) struct MapSlot {
    pub map: Map,
    pub binding: u32,
    pub kind: TextureKind,
    pub path: fn(&MaterialAsset) -> Option<&MapRef>,
    pub off_while_missing: Option<fn(&mut MaterialData) -> &mut u32>,
}

/// Maps past emissive count down from 15, so a new one takes 9; the shader slots keep 7 up.
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
        path: |m| {
            let d = m.detail.as_ref().filter(|d| d.map.is_some())?;
            d.mask.as_ref()
        },
        off_while_missing: Some(detail_mask_flag),
    },
    MapSlot {
        map: Map::SpecularColor,
        binding: 10,
        kind: TextureKind::Colour,
        path: |m| m.specular_color_map.as_ref(),
        off_while_missing: Some(specular_color_flag),
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

fn specular_color_flag(record: &mut MaterialData) -> &mut u32 {
    &mut record.level_maps.use_specular_color_map
}

fn height_flag(record: &mut MaterialData) -> &mut u32 {
    &mut record.level_maps.use_height_map
}

fn detail_mask_flag(record: &mut MaterialData) -> &mut u32 {
    &mut record.detail.use_detail_mask
}

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
                "two material-group textures share a binding"
            );
            j += 1;
        }
        i += 1;
    }
};

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

pub(crate) fn map_sampler_binding(binding: u32) -> u32 {
    match binding {
        0 => 1,
        b => textures::sampler_binding(b),
    }
}

/// What the material-group texture at `index` (in [`material_texture_paths`] order) holds:
/// its row's kind, and data for an extra shader texture slot.
pub(crate) fn map_kind(index: usize) -> TextureKind {
    MAP_SLOTS.get(index).map_or(TextureKind::Data, |s| s.kind)
}

pub(crate) fn material_maps(
    material: Option<&MaterialAsset>,
) -> impl Iterator<Item = (MapRef, TextureKind)> {
    material_texture_paths(material)
        .into_iter()
        .enumerate()
        .filter_map(|(i, path)| Some((path?, map_kind(i))))
}

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
        height_min_steps: m.height_steps().0 as f32,
        height_max_steps: m.height_steps().1 as f32,
        use_specular_color_map: u32::from(m.specular_color_map.is_some()),
    }
}

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
