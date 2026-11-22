//! Scenes store a path-based reference (`path::sub_object`), never GPU buffers;
//! the geometry is re-imported on load, as a primitive is rebuilt from its
//! `primitive_type`. Paths, not GUIDs, keep references readable and diffable for
//! an agent/PR-driven engine; the `.meta` sidecar carries import settings only,
//! never identity. A sim module: re-imports are deterministic for the same file.

#![deny(clippy::unwrap_used)]

pub mod anim_data;
pub mod animation_graph;
pub mod audio;
pub mod clusters;
pub mod convert;
pub mod cook;
pub mod flat_normals;
pub mod gltf;
pub mod humanoid;
pub mod import;
pub mod import_cache;
pub mod lightmap_page;
pub mod lightmap_uv;
pub mod lod;
pub mod lut;
pub mod manifest;
pub mod material_range;
pub mod mesh_data;
pub mod model_node;
pub mod obj_import;
pub mod reference;
pub mod scale_factor;
pub mod sidecar;
pub mod tangents;
pub mod texture_import;

pub use anim_data::{AnimationClip, Interpolation, JointTrack, Track};
pub use animation_graph::{
    AnimationGraph, Condition, GraphEdge, GraphError, GraphNode, NumericOp, ParameterDeclaration,
};
pub use convert::texture_ref::MapRef;
pub use import_cache::{imported, Imported, MeshRevision};
pub use lightmap_uv::LightmapUvSettings;
pub use lut::Lut;
pub use manifest::{build_manifest, AssetEntry, AssetManifest, SubObjectEntry};
pub use material_range::MaterialRange;
pub use mesh_data::{ImportedAsset, JointTransform, MaterialData, SkinData, SubMesh, Vertex};
pub use model_node::{ModelNode, NodeBone};
pub use reference::{AssetRef, REF_SEPARATOR};
pub use sidecar::{import_and_sync_sidecar, ImportSettings, MeshColliderKind, SubObjectCollider};

use rusty_core::paths;
use std::path::Path;
use tangents::Missing;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    UnsupportedFormat(String),
    Parse(String),
    /// `'<reference>': <why>`.
    BadReference(String),
    Sidecar(String),
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::UnsupportedFormat(s) => write!(f, "unsupported asset format: {s}"),
            ImportError::Parse(s) => write!(f, "failed to parse asset: {s}"),
            ImportError::BadReference(s) => write!(f, "bad asset reference: {s}"),
            ImportError::Sidecar(s) => write!(f, "asset sidecar error: {s}"),
        }
    }
}

impl std::error::Error for ImportError {}

/// Lowercase; the one list the editor reads. `fbx` and `blend` go through a cached
/// glTF ([`convert`]).
pub const MODEL_EXTENSIONS: &[&str] = &["gltf", "glb", "obj", "fbx", "blend"];

/// Lowercase; matches the `image` features in `Cargo.toml`.
pub const IMAGE_EXTENSIONS: &[&str] = &["png", "tga", "jpg", "jpeg"];

/// Lowercase. A `.png` is a LUT only as a strip (`N² × N` pixels); any other is
/// refused, naming its size.
pub const LUT_EXTENSIONS: &[&str] = &["cube", "png"];

/// Case-insensitive.
pub fn is_model_path(path: impl AsRef<Path>) -> bool {
    MODEL_EXTENSIONS.contains(&paths::extension(path).as_str())
}

/// Case-insensitive.
pub fn is_image_path(path: impl AsRef<Path>) -> bool {
    IMAGE_EXTENSIONS.contains(&paths::extension(path).as_str())
}

/// Scale Factor applies before Generate Lightmap UVs, so the unwrap sees the
/// scaled model. Always parses; scene builds use [`imported`].
pub fn import_file(path: &Path) -> Result<ImportedAsset, ImportError> {
    import_with(path, &read_target(path)?, &settings_of(path))
}

/// For a converted source, its cached glTF, brought up to date first.
fn read_target(path: &Path) -> Result<std::path::PathBuf, ImportError> {
    match convert::converter_for(path) {
        Some(_) => convert::cached_gltf(path),
        None => Ok(path.to_path_buf()),
    }
}

fn settings_of(path: &Path) -> ImportSettings {
    sidecar::load(path).unwrap_or_else(|e| {
        log::warn!(
            "[Asset] {}: {e}; importing with default settings",
            path.display()
        );
        ImportSettings::default()
    })
}

fn import_with(
    path: &Path,
    read: &Path,
    settings: &ImportSettings,
) -> Result<ImportedAsset, ImportError> {
    let mut asset = import_own(path, read, settings)?;
    humanoid::borrow_clips(path, &mut asset, &settings.humanoid, borrowed_source);
    Ok(asset)
}

pub(crate) fn import_own_file(path: &Path) -> Result<ImportedAsset, ImportError> {
    import_own(path, &read_target(path)?, &settings_of(path))
}

/// [`import_with`] without borrowed clips.
fn import_own(
    path: &Path,
    read: &Path,
    settings: &ImportSettings,
) -> Result<ImportedAsset, ImportError> {
    cook::import(path, read, settings, |missing| {
        parse_file(path, read, missing)
    })
}

/// Never includes the source's own borrowed clips.
fn borrowed_source(from: &str) -> Result<Option<humanoid::borrow::Source>, ImportError> {
    let path = Path::new(from);
    let settings = settings_of(path);
    let asset = import_own(path, &read_target(path)?, &settings)?;
    Ok(humanoid::borrow::Source::of(
        from,
        asset,
        &settings.humanoid,
    ))
}

fn parse_file(path: &Path, read: &Path, missing: Missing) -> Result<ImportedAsset, ImportError> {
    match paths::extension(path).as_str() {
        "gltf" | "glb" => gltf::import::import_as(read, None, missing),
        "obj" => obj_import::import_as(read, missing),
        _ if convert::converter_for(path).is_some() => {
            let source = path.to_string_lossy();
            gltf::import::import_as(read, Some(&source), missing)
        }
        other => Err(ImportError::UnsupportedFormat(other.to_string())),
    }
}

pub fn import_sub_mesh(reference: &str) -> Result<SubMesh, ImportError> {
    let asset_ref = AssetRef::parse(reference)?;
    let asset = imported(Path::new(&asset_ref.path))?;
    asset
        .sub_mesh(&asset_ref.sub_object)
        .cloned()
        .ok_or_else(|| reference::bad(reference, "the file has no such sub-object"))
}

#[cfg(test)]
pub(crate) mod flat_normals_tests;
#[cfg(test)]
mod manifest_tests;
#[cfg(test)]
mod multi_material_tests;
#[cfg(test)]
mod ref_tests;
#[cfg(test)]
mod tests;
