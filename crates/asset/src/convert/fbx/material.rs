//! Classic materials (Lambert, Phong, unknown) follow URP's FBX import
//! (`FBXMaterialDescriptionPreprocessor.cs`): Unity stores colours gamma-encoded,
//! a glTF factor is linear. Bound to a texture, diffuse is white × `DiffuseFactor`
//! and emissive is white (the factor only switches it on); unbound, the colour
//! (× the emissive factor), linearised. Other shading models are colour × weight.

use super::glb;
use super::texture::Textures;
use crate::ImportError;
use rusty_core::color::gamma_to_linear;

pub fn material(
    material: &ufbx::Material,
    textures: &mut Textures,
) -> Result<glb::Material, ImportError> {
    let pbr = &material.pbr;
    let factor = |map: &ufbx::MaterialMap| value_or(map, 1.0);
    // A colour left out is glTF's default: white base, black emission.
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
        // A colour bound to a texture is the exporter's placeholder, not a tint. A texture not
        // written reads by the unbound rule, as URP's import does when one is missing, so a
        // skipped emission mask's placeholder glows.
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
