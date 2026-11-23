use super::uri;
use crate::convert::texture_ref;
use crate::mesh_data::MaterialData;
use crate::MapRef;
use rusty_core::paths;
use std::path::Path;

pub(super) fn material_data(
    material: gltf::Material,
    base_dir: &Path,
    source: Option<&str>,
) -> MaterialData {
    let pbr = material.pbr_metallic_roughness();
    let specular = material.specular();
    for (map, set) in off_uv0(&material) {
        let name = material.name().unwrap_or("<unnamed>");
        log::warn!(
            "[Asset] glTF material `{name}`: its {map} reads TEXCOORD_{set}, but every map is read through UV0 (TEXCOORD_0)"
        );
    }
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
        // An ORM image shared with metallic-roughness resolves to the same path: one upload.
        occlusion_map: material
            .occlusion_texture()
            .and_then(|t| texture_path(&t.texture(), base_dir, source))
            .map(MapRef::from),
        occlusion_strength: material.occlusion_texture().map_or(1.0, |t| t.strength()),
        double_sided: material.double_sided() && !from_blend(source),
        ior: material.ior().unwrap_or(1.5),
        specular: specular.as_ref().map_or(1.0, |s| s.specular_factor()),
        specular_map: specular
            .as_ref()
            .and_then(|s| s.specular_texture())
            .and_then(|t| texture_path(&t.texture(), base_dir, source))
            .map(MapRef::from),
        specular_color: specular
            .as_ref()
            .map_or([1.0; 3], |s| s.specular_color_factor()),
        specular_color_map: specular
            .as_ref()
            .and_then(|s| s.specular_color_texture())
            .and_then(|t| texture_path(&t.texture(), base_dir, source))
            .map(MapRef::from),
    }
}

/// The renderer reads every map through UV0, so a texture authored on another `texCoord`
/// set lands wrong: the import warns once per material and map.
pub(super) fn off_uv0(material: &gltf::Material) -> Vec<(&'static str, u32)> {
    let pbr = material.pbr_metallic_roughness();
    let specular = material.specular();
    let sets = [
        (
            "base colour map",
            pbr.base_color_texture().map(|t| t.tex_coord()),
        ),
        (
            "metallic-roughness map",
            pbr.metallic_roughness_texture().map(|t| t.tex_coord()),
        ),
        (
            "normal map",
            material.normal_texture().map(|t| t.tex_coord()),
        ),
        (
            "occlusion map",
            material.occlusion_texture().map(|t| t.tex_coord()),
        ),
        (
            "emissive map",
            material.emissive_texture().map(|t| t.tex_coord()),
        ),
        (
            "specular map",
            specular
                .as_ref()
                .and_then(|s| s.specular_texture())
                .map(|t| t.tex_coord()),
        ),
        (
            "specular colour map",
            specular
                .and_then(|s| s.specular_color_texture())
                .map(|t| t.tex_coord()),
        ),
    ];
    sets.into_iter()
        .filter_map(|(map, set)| set.filter(|&s| s != 0).map(|s| (map, s)))
        .collect()
}

/// Whether the glTF is a `.blend`'s conversion, whose `doubleSided` says nothing: Blender's
/// exporter writes it whenever Backface Culling is off (the default). A `.blend` imports
/// single-sided, as Unity's does; a `.gltf`/`.glb` keeps its own.
fn from_blend(source: Option<&str>) -> bool {
    source.is_some_and(|s| paths::extension(s) == "blend")
}

#[cfg(test)]
#[path = "material_tests.rs"]
mod tests;

/// A named image a conversion wrote into the cache is a reference to it; one lying
/// elsewhere stays the path it is. An embedded image returns `None`.
fn texture_path(texture: &gltf::Texture, base_dir: &Path, source: Option<&str>) -> Option<String> {
    let image = texture.source();
    let file = match image.source() {
        gltf::image::Source::Uri { uri, .. } => uri::file(base_dir, uri)?,
        gltf::image::Source::View { .. } => return None,
    };
    if let (Some(source), Some(name)) = (source, image.name()) {
        if file.starts_with(paths::fold(base_dir)) {
            return Some(texture_ref::reference(source, name));
        }
    }
    Some(file.to_string_lossy().into_owned())
}
