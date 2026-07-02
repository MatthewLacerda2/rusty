//! src/asset/ — the headless 3D-asset importer (issue #74).
//!
//! Turns an authored source file (glTF 2.0 `.gltf`/`.glb`, or Wavefront `.obj`)
//! into addressable, scene-referenceable sub-meshes — WITHOUT ever inlining GPU
//! buffers into the scene document. Scenes store a path-based REFERENCE
//! (`path::sub_object`); the geometry is re-imported on scene load, exactly like a
//! primitive mesh is rebuilt from its `primitive_type`.
//!
//! Reference identity is path-based by design (issue #74): `crates.glb::Barrel` is
//! human-readable and diffable, which suits an agent/PR-driven engine better than
//! opaque GUIDs. The `<file>.meta` sidecar carries import SETTINGS only, never
//! identity (see `sidecar`).
//!
//! Purity: this module depends only on `glam` + the `gltf`/`tobj`/`serde` crates —
//! never wgpu/egui/mlua — so it stays a pure data transform. It lives outside the
//! determinism-guarded sim trees (`app`/`scripting`/`physics`/`navigation`); a
//! scene-load re-import is deterministic given the same file.

pub mod anim_data;
pub mod gltf_anim;
pub mod gltf_import;
pub mod gltf_skin;
pub mod manifest;
pub mod mesh_data;
pub mod obj_import;
pub mod sidecar;
pub mod tangents;

pub use anim_data::{AnimationClip, Interpolation, JointTrack, Track};
pub use manifest::{build_manifest, AssetEntry, AssetManifest, SubObjectEntry};
pub use mesh_data::{ImportedAsset, JointTransform, MaterialData, MeshVertex, SkinData, SubMesh};
pub use sidecar::{import_and_sync_sidecar, ImportSettings, MeshColliderKind};

use std::path::Path;

/// The separator between a source path and a sub-object id in a scene reference:
/// `crates.glb::Barrel`.
pub const REF_SEPARATOR: &str = "::";

/// Anything that can go wrong importing a source file. Kept small and string-y:
/// the importer is a leaf, and callers only ever surface these to a log/console.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// The file extension isn't a recognised 3D source format.
    UnsupportedFormat(String),
    /// The underlying parser (gltf/tobj) rejected the file.
    Parse(String),
    /// The reference string wasn't `path::sub_object`, or the sub-object is absent.
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

/// True if `path` looks like a source 3D asset this importer can read
/// (`.gltf`/`.glb`/`.obj`). Case-insensitive.
pub fn is_model_path(path: &str) -> bool {
    matches!(extension(path).as_str(), "gltf" | "glb" | "obj")
}

fn extension(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

/// Import a whole source file into its addressable sub-meshes + materials,
/// dispatching on the extension. Pure: no sidecar I/O (see
/// [`import_and_sync_sidecar`] for the variant that maintains the `.meta` cache).
pub fn import_file(path: &Path) -> Result<ImportedAsset, ImportError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "gltf" | "glb" => gltf_import::import(path),
        "obj" => obj_import::import(path),
        other => Err(ImportError::UnsupportedFormat(other.to_string())),
    }
}

/// A parsed scene reference: a source file path and the sub-object within it.
/// This is the path-based identity an `"Asset"` mesh reference rehydrates from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetRef {
    pub path: String,
    pub sub_object: String,
}

impl AssetRef {
    /// Build the canonical reference string `path::sub_object`.
    pub fn to_string_ref(&self) -> String {
        format!("{}{}{}", self.path, REF_SEPARATOR, self.sub_object)
    }

    /// Parse `path::sub_object`. The path may itself contain no `::` (paths don't),
    /// so we split on the LAST separator to be safe.
    pub fn parse(reference: &str) -> Result<AssetRef, ImportError> {
        match reference.rsplit_once(REF_SEPARATOR) {
            Some((path, sub)) if !path.is_empty() && !sub.is_empty() => Ok(AssetRef {
                path: path.to_string(),
                sub_object: sub.to_string(),
            }),
            _ => Err(ImportError::BadReference(reference.to_string())),
        }
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
        .ok_or_else(|| ImportError::BadReference(reference.to_string()))
}

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod fixtures_anim;
#[cfg(test)]
mod gltf_import_tests;
#[cfg(test)]
mod manifest_tests;
#[cfg(test)]
mod tests;
