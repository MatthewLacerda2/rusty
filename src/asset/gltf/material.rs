//! One glTF material as the pure-data [`MaterialData`]: its factors and the
//! textures it samples, as paths or, for a converted source, references.

use super::uri;
use crate::asset::convert::texture_ref;
use crate::asset::mesh_data::MaterialData;
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
            .and_then(|t| texture_path(&t.texture(), base_dir, source)),
        metallic_roughness_map: pbr
            .metallic_roughness_texture()
            .and_then(|t| texture_path(&t.texture(), base_dir, source)),
        normal_map: material
            .normal_texture()
            .and_then(|t| texture_path(&t.texture(), base_dir, source)),
        emissive_map: material
            .emissive_texture()
            .and_then(|t| texture_path(&t.texture(), base_dir, source)),
        emissive: material.emissive_factor(),
    }
}

/// Resolve a glTF texture's image to a file path, anchored at the glTF file's
/// directory; a named image of the glTF a conversion of `source` wrote is the
/// reference to it instead (`convert::texture_ref`). Only the EXTERNAL-URI case
/// is handled: an embedded image (`Source::View`, a buffer view / data URI) returns `None` — extracting its pixels
/// to disk is a documented follow-up, out of scope for #203. The URI is
/// percent-decoded and folded (`uri::file`); one that doesn't decode is warned
/// about and dropped.
fn texture_path(texture: &gltf::Texture, base_dir: &Path, source: Option<&str>) -> Option<String> {
    let image = texture.source();
    if let (Some(source), Some(name)) = (source, image.name()) {
        return Some(texture_ref::reference(source, name));
    }
    match image.source() {
        gltf::image::Source::Uri { uri, .. } => {
            Some(uri::file(base_dir, uri)?.to_string_lossy().into_owned())
        }
        // Embedded image: no on-disk path to reference (follow-up).
        gltf::image::Source::View { .. } => None,
    }
}
