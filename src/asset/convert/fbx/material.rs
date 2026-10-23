//! An FBX material as a glTF metallic-roughness one.
//!
//! ufbx fills `pbr` for every shading model (a classic Phong/Lambert material
//! included), so this reads only that view. Its textures come from `texture`: a
//! file on disk is referenced where it lies, one embedded in the FBX is extracted
//! beside the written `.glb` (#1082).
//!
//! A classic material's colours (Lambert, Phong, and what ufbx can't tell apart
//! from them) follow URP's FBX import (`FBXMaterialDescriptionPreprocessor.cs`, in
//! `com.unity.render-pipelines.universal/Editor/AssetPostProcessors`, #1293). Unity
//! stores a material colour gamma-encoded and hands the shader its linear value,
//! while a glTF factor is linear, so each colour here is what URP's shader sees:
//! - a diffuse colour bound to a texture is white × `DiffuseFactor`, linearised;
//! - an unbound one is `DiffuseColor` itself, no factor: URP sets its `.gamma`,
//!   which the shader linearises back;
//! - an emissive colour bound to a texture is white, the map shows as itself, and
//!   `EmissiveFactor` only switches emission on (above zero) or off;
//! - an unbound one is `EmissiveColor` × `EmissiveFactor`, linearised.
//!
//! A texture that isn't written leaves its colour as the unbound rule reads it, so
//! a skipped emission mask's placeholder colour glows: URP's import does the same
//! when the texture is missing. Every other shading model (Arnold and OSL Standard
//! Surface, 3ds Max Physical, OpenPBR, …) is colour × its weight, as Unity's own
//! preprocessors for them apply the base weight.

use super::glb;
use super::texture::Textures;
use crate::asset::ImportError;
use crate::core::color::gamma_to_linear;

/// `material` as a glTF material, its textures resolved through `textures`.
pub fn material(
    material: &ufbx::Material,
    textures: &mut Textures,
) -> Result<glb::Material, ImportError> {
    let pbr = &material.pbr;
    let factor = |map: &ufbx::MaterialMap| value_or(map, 1.0);
    // A colour the file leaves out is glTF's default: white for the base, which a
    // texture alone then shows as itself (#1280), and black for emission.
    let rgb = |map: &ufbx::MaterialMap, default: f32| {
        let v = &map.value_vec4;
        if map.has_value {
            [v.x, v.y, v.z].map(|c| c as f32)
        } else {
            [default; 3]
        }
    };
    let base_color_texture = textures.texture(&pbr.base_color)?;
    let normal_texture = textures.texture(&pbr.normal_map)?;
    let emissive_texture = textures.texture(&pbr.emission_color)?;
    let (base, emission) = (factor(&pbr.base_factor), factor(&pbr.emission_factor));
    let ([r, g, b], emissive) = if classic(material.shader_type) {
        // A colour bound to a texture is the exporter's placeholder, often grey or
        // black, not a tint (#1287); a placeholder under an emissive mask would
        // make the whole mesh glow.
        let base_color = match base_color_texture {
            Some(_) => [gamma_to_linear(base); 3],
            None => rgb(&pbr.base_color, 1.0),
        };
        let emissive = match emissive_texture {
            Some(_) => [if emission > 0.0 { 1.0 } else { 0.0 }; 3],
            None => rgb(&pbr.emission_color, 0.0).map(|c| gamma_to_linear(c * emission)),
        };
        (base_color, emissive)
    } else {
        let weighted = |map, default, weight| rgb(map, default).map(|c| c * weight);
        (
            weighted(&pbr.base_color, 1.0, base),
            weighted(&pbr.emission_color, 0.0, emission),
        )
    };
    Ok(glb::Material {
        name: material.element.name.to_string(),
        base_color: [r, g, b, factor(&pbr.opacity)],
        metallic: value_or(&pbr.metalness, 0.0),
        roughness: value_or(&pbr.roughness, 0.5),
        emissive,
        base_color_texture,
        normal_texture,
        emissive_texture,
    })
}

/// Whether URP's classic FBX rules read a material of `shader`: ufbx's
/// Lambert and Phong, Blender's Phong export, and a material it can't place.
fn classic(shader: ufbx::ShaderType) -> bool {
    use ufbx::ShaderType::{BlenderPhong, FbxLambert, FbxPhong, Unknown};
    matches!(shader, FbxLambert | FbxPhong | BlenderPhong | Unknown)
}

fn value_or(map: &ufbx::MaterialMap, default: f32) -> f32 {
    if map.has_value {
        map.value_vec4.x as f32
    } else {
        default
    }
}

#[cfg(test)]
#[path = "material_tests.rs"]
mod tests;
