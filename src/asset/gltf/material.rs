//! One glTF material as the pure-data [`MaterialData`]: its factors and the
//! textures it samples, as paths or, for a converted source, references.

use super::uri;
use crate::asset::convert::texture_ref;
use crate::asset::mesh_data::MaterialData;
use crate::asset::MapRef;
use crate::core::paths;
use std::path::Path;

/// Read one glTF material into the pure-data [`MaterialData`]: the metallic-roughness
/// factors plus the resolved file paths of its external textures (`base_dir` anchors
/// relative URIs), or references into `source` (see [`texture_path`]). Embedded
/// images yield `None` maps.
pub(super) fn material_data(
    material: gltf::Material,
    base_dir: &Path,
    source: Option<&str>,
) -> MaterialData {
    let pbr = material.pbr_metallic_roughness();
    MaterialData {
        name: material.name().unwrap_or("").to_string(),
        base_color: pbr.base_color_factor(),
        metallic: pbr.metallic_factor(),
        roughness: pbr.roughness_factor(),
        base_color_map: pbr
            .base_color_texture()
            .and_then(|t| texture_path(&t.texture(), base_dir, source))
            .map(MapRef::from),
        metallic_roughness_map: pbr
            .metallic_roughness_texture()
            .and_then(|t| texture_path(&t.texture(), base_dir, source))
            .map(MapRef::from),
        normal_map: material
            .normal_texture()
            .and_then(|t| texture_path(&t.texture(), base_dir, source))
            .map(MapRef::from),
        emissive_map: material
            .emissive_texture()
            .and_then(|t| texture_path(&t.texture(), base_dir, source))
            .map(MapRef::from),
        emissive: material.emissive_factor(),
        double_sided: material.double_sided() && !from_blend(source),
    }
}

/// Whether the glTF is a `.blend`'s conversion, whose `doubleSided` says nothing
/// (#1615): Blender's exporter writes it on every material whose Backface Culling
/// is off, Blender's default, which would draw every Blender model's back faces. So
/// a `.blend` imports single-sided, as Unity's `.blend` import does (through FBX,
/// which has no such flag), and *Render Face* is set after import. A `.gltf` or
/// `.glb` authored as such keeps its `doubleSided`, as Unity's glTFast reads it.
fn from_blend(source: Option<&str>) -> bool {
    source.is_some_and(|s| paths::extension(s) == "blend")
}

#[cfg(test)]
#[path = "material_tests.rs"]
mod tests;

/// Resolve a glTF texture's image to a file path, anchored at the glTF file's
/// directory; a named image a conversion of `source` wrote, which lies in the
/// cache's folder, is the reference to it instead (`convert::texture_ref`). One
/// that lies elsewhere is a file on disk and stays the path it is, whatever the
/// converter named it (#1072). Only the EXTERNAL-URI case is handled: an embedded
/// image (`Source::View`, a buffer view / data URI) returns `None` — extracting
/// its pixels to disk is a documented follow-up, out of scope for #203. The URI is
/// percent-decoded and folded (`uri::file`); one that doesn't decode is warned
/// about and dropped.
fn texture_path(texture: &gltf::Texture, base_dir: &Path, source: Option<&str>) -> Option<String> {
    let image = texture.source();
    let file = match image.source() {
        gltf::image::Source::Uri { uri, .. } => uri::file(base_dir, uri)?,
        // Embedded image: no on-disk path to reference (follow-up).
        gltf::image::Source::View { .. } => return None,
    };
    if let (Some(source), Some(name)) = (source, image.name()) {
        if file.starts_with(paths::fold(base_dir)) {
            return Some(texture_ref::reference(source, name));
        }
    }
    Some(file.to_string_lossy().into_owned())
}
