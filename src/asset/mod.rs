//! src/asset/ — the headless 3D-asset importer (issue #74).
//!
//! Turns an authored source file (glTF 2.0 `.gltf`/`.glb`, Wavefront `.obj`, or an
//! `.fbx` read through the glTF it is converted to, `convert`) into addressable, scene-referenceable sub-meshes — WITHOUT ever inlining GPU
//! buffers into the scene document. Scenes store a path-based REFERENCE
//! (`path::sub_object`); the geometry is re-imported on scene load, exactly like a
//! primitive mesh is rebuilt from its `primitive_type`.
//!
//! Reference identity is path-based by design (issue #74): `crates.glb::Barrel` is
//! human-readable and diffable, which suits an agent/PR-driven engine better than
//! opaque GUIDs. The `<file>.meta` sidecar carries import SETTINGS only, never
//! identity (see `sidecar`).
//!
//! Audio import (`audio`, #385) lives here too: MP3 is a source format, converted to
//! WAV on arrival, and the one decoder every clip goes through.
//!
//! Purity: this module depends only on `glam` + the `gltf`/`tobj`/`ufbx`/`serde`/`symphonia`
//! crates — never wgpu/egui/mlua — so it stays a pure data transform. It lives outside the
//! determinism-guarded sim trees (`app`/`scripting`/`physics`/`navigation`); a
//! scene-load re-import is deterministic given the same file.

// Panic-free sim (#195, #962): bare `.unwrap()` is denied in every sim module (use `?`
// or a documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

pub mod anim_data;
pub mod animation_graph;
pub mod audio;
pub mod convert;
pub mod flat_normals;
pub mod gltf;
pub mod lightmap_uv;
pub mod lod;
pub mod manifest;
pub mod material_range;
pub mod mesh_data;
pub mod obj_import;
pub mod reference;
pub mod sidecar;
pub mod tangents;

pub use anim_data::{AnimationClip, Interpolation, JointTrack, Track};
pub use animation_graph::{
    AnimationGraph, Condition, GraphEdge, GraphError, GraphNode, NumericOp, ParameterDeclaration,
};
pub use convert::texture_ref::MapRef;
pub use lightmap_uv::LightmapUvSettings;
pub use manifest::{build_manifest, AssetEntry, AssetManifest, SubObjectEntry};
pub use material_range::MaterialRange;
pub use mesh_data::{ImportedAsset, JointTransform, MaterialData, MeshVertex, SkinData, SubMesh};
pub use reference::{AssetRef, REF_SEPARATOR};
pub use sidecar::{import_and_sync_sidecar, ImportSettings, MeshColliderKind};

use crate::core::paths;
use std::path::Path;

/// Anything that can go wrong importing a source file. Kept small and string-y:
/// the importer is a leaf, and callers only ever surface these to a log/console.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// The file extension isn't a recognised 3D source format.
    UnsupportedFormat(String),
    /// The underlying parser (gltf/tobj) rejected the file.
    Parse(String),
    /// The reference string wasn't `path::sub_object` ([`AssetRef`]), or the
    /// sub-object is absent: `'<reference>': <why>`.
    BadReference(String),
    /// Reading/writing the `.meta` sidecar failed.
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

/// The source model formats [`import_file`] reads, lowercase (#983, #994). The one
/// list: the editor's content grid, inspector and preview read it rather than
/// spelling their own, so nothing offers a model the importer would reject. `fbx`
/// and `blend` are read through a cached glTF ([`convert`], #1024, #1072).
pub const MODEL_EXTENSIONS: &[&str] = &["gltf", "glb", "obj", "fbx", "blend"];

/// The image formats a texture is decoded from, lowercase (#994): what the
/// renderer's `image` decoders are built with (`Cargo.toml`'s `image` features).
pub const IMAGE_EXTENSIONS: &[&str] = &["png", "tga", "jpg", "jpeg"];

/// True if `path` looks like a source 3D asset this importer can read
/// ([`MODEL_EXTENSIONS`]). Case-insensitive.
pub fn is_model_path(path: impl AsRef<Path>) -> bool {
    MODEL_EXTENSIONS.contains(&paths::extension(path).as_str())
}

/// True if `path` looks like a texture image ([`IMAGE_EXTENSIONS`]).
/// Case-insensitive.
pub fn is_image_path(path: impl AsRef<Path>) -> bool {
    IMAGE_EXTENSIONS.contains(&paths::extension(path).as_str())
}

/// Import a whole source file into its addressable sub-meshes + materials,
/// dispatching on the extension, then apply the import settings its `.meta` sidecar
/// records (Generate Lightmap UVs, #831). Reads the sidecar, never writes it (see
/// [`import_and_sync_sidecar`] for the variant that maintains the `.meta` cache); a
/// file with no sidecar imports with the defaults.
pub fn import_file(path: &Path) -> Result<ImportedAsset, ImportError> {
    let mut asset = parse_file(path)?;
    let settings = sidecar::load(path).unwrap_or_else(|e| {
        log::warn!(
            "[Asset] {}: {e}; importing with default settings",
            path.display()
        );
        ImportSettings::default()
    });
    if settings.lightmap_uvs.generate {
        for sub in &mut asset.sub_meshes {
            lightmap_uv::apply(sub, &settings.lightmap_uvs);
        }
    }
    Ok(asset)
}

/// Parse a source file into sub-meshes + materials by its extension, as authored.
fn parse_file(path: &Path) -> Result<ImportedAsset, ImportError> {
    match paths::extension(path).as_str() {
        "gltf" | "glb" => gltf::import::import(path),
        "obj" => obj_import::import(path),
        _ if convert::converter_for(path).is_some() => {
            let source = path.to_string_lossy();
            gltf::import::import_from(&convert::cached_gltf(path)?, Some(&source))
        }
        other => Err(ImportError::UnsupportedFormat(other.to_string())),
    }
}

/// Re-import just the sub-mesh a reference points at. This is the hot path scene
/// rehydration calls: parse the reference, import the file, pull out the one
/// addressable sub-object.
pub fn import_sub_mesh(reference: &str) -> Result<SubMesh, ImportError> {
    let asset_ref = AssetRef::parse(reference)?;
    let asset = import_file(Path::new(&asset_ref.path))?;
    asset
        .sub_mesh(&asset_ref.sub_object)
        .cloned()
        .ok_or_else(|| reference::bad(reference, "the file has no such sub-object"))
}

#[cfg(test)]
pub(crate) mod fixtures;
#[cfg(test)]
pub(crate) mod fixtures_anim;
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
