//! An FBX material as a glTF metallic-roughness one.
//!
//! ufbx fills `pbr` for every shading model (a classic Phong/Lambert material
//! included), so this reads only that view. Its textures come from `texture`: a
//! file on disk is referenced where it lies, one embedded in the FBX is extracted
//! beside the written `.glb` (#1082).

use super::glb;
use super::texture::Textures;
use crate::asset::ImportError;

/// `material` as a glTF material, its textures resolved through `textures`.
pub fn material(
    material: &ufbx::Material,
    textures: &mut Textures,
) -> Result<glb::Material, ImportError> {
    let pbr = &material.pbr;
    let factor = |map: &ufbx::MaterialMap| value_or(map, 1.0);
    let rgb = |map: &ufbx::MaterialMap, scale: f32| {
        let v = &map.value_vec4;
        [v.x, v.y, v.z].map(|c| c as f32 * scale)
    };
    let [r, g, b] = rgb(&pbr.base_color, factor(&pbr.base_factor));
    let emissive = if pbr.emission_color.has_value {
        rgb(&pbr.emission_color, factor(&pbr.emission_factor))
    } else {
        [0.0; 3]
    };
    Ok(glb::Material {
        name: material.element.name.to_string(),
        base_color: [r, g, b, factor(&pbr.opacity)],
        metallic: value_or(&pbr.metalness, 0.0),
        roughness: value_or(&pbr.roughness, 0.5),
        emissive,
        base_color_texture: textures.uri(&pbr.base_color)?,
        normal_texture: textures.uri(&pbr.normal_map)?,
    })
}

fn value_or(map: &ufbx::MaterialMap, default: f32) -> f32 {
    if map.has_value {
        map.value_vec4.x as f32
    } else {
        default
    }
}
