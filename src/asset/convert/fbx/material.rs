//! An FBX material as a glTF metallic-roughness one.
//!
//! ufbx fills `pbr` for every shading model (a classic Phong/Lambert material
//! included), so this reads only that view. Textures are kept when they are files
//! on disk, as URIs relative to the written `.glb`; a texture embedded in the FBX
//! is not extracted yet.

use super::glb;
use crate::core::paths;
use std::path::{Component, Path, PathBuf};

/// `material` as a glTF material; `out_dir` is where the `.glb` is written.
pub fn material(material: &ufbx::Material, out_dir: &Path) -> glb::Material {
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
    glb::Material {
        name: material.element.name.to_string(),
        base_color: [r, g, b, factor(&pbr.opacity)],
        metallic: value_or(&pbr.metalness, 0.0),
        roughness: value_or(&pbr.roughness, 0.5),
        emissive,
        base_color_texture: texture_uri(&pbr.base_color, out_dir),
        normal_texture: texture_uri(&pbr.normal_map, out_dir),
    }
}

fn value_or(map: &ufbx::MaterialMap, default: f32) -> f32 {
    if map.has_value {
        map.value_vec4.x as f32
    } else {
        default
    }
}

/// The URI, relative to `out_dir`, of the file `map`'s texture names, if that
/// file exists.
fn texture_uri(map: &ufbx::MaterialMap, out_dir: &Path) -> Option<String> {
    let texture = map.texture.as_ref()?;
    let file = [&texture.filename, &texture.absolute_filename]
        .into_iter()
        .map(|name| PathBuf::from(name.as_ref() as &str))
        .find(|path| !path.as_os_str().is_empty() && path.is_file());
    let Some(file) = file else {
        log::warn!(
            "[Asset] FBX texture `{}` is not a file on disk (embedded textures are not extracted yet)",
            texture.element.name
        );
        return None;
    };
    relative_uri(out_dir, &file)
}

/// `target` as a `/`-separated path from directory `from`, both resolved first.
pub fn relative_uri(from: &Path, target: &Path) -> Option<String> {
    let from = paths::canonicalize(from).ok()?;
    let target = paths::canonicalize(target).ok()?;
    let from: Vec<Component> = from.components().collect();
    let target: Vec<Component> = target.components().collect();
    let shared = from.iter().zip(&target).take_while(|(a, b)| a == b).count();
    let up = std::iter::repeat_n("..".to_string(), from.len() - shared);
    let down = target[shared..]
        .iter()
        .map(|c| c.as_os_str().to_string_lossy().into_owned());
    Some(up.chain(down).collect::<Vec<_>>().join("/"))
}
