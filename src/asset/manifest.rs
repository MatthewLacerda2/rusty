//! src/asset/manifest.rs — the project-wide, reference-ready asset manifest (#179).
//!
//! The importer already turns ONE source file into addressable sub-objects
//! ([`ImportedAsset`](super::ImportedAsset) / [`SubMesh`]). This module surfaces
//! that inventory across a whole asset root: walk the directory for model files,
//! import each, and emit a flat, structured catalogue of `path::sub_object`
//! references plus each sub-object's footprint (AABB) and material count. It is the
//! "see what I can place" half of authoring — enough for an agent to pick "a sedan"
//! and know its size for non-overlapping layout.
//!
//! Purity: this stays a pure data transform (glam + the importer only, no
//! wgpu/egui/mlua), and reference strings round-trip through [`AssetRef`] /
//! [`import_sub_mesh`](super::import_sub_mesh) by construction — every entry's
//! `reference` is built from the same `path::sub_object` shape the rehydration path
//! parses.

use std::path::Path;

use glam::Vec3;

use super::{import_file, is_model_path, AssetRef, SubMesh};

/// One addressable sub-object within a source file, with the footprint an agent
/// needs to place it without overlap.
#[derive(Clone, Debug, PartialEq)]
pub struct SubObjectEntry {
    /// The sub-object's id (the part after `::`), e.g. `Sedan`.
    pub id: String,
    /// The canonical `path::sub_object` reference, ready to hand to
    /// [`import_sub_mesh`](super::import_sub_mesh) / [`AssetRef::parse`].
    pub reference: String,
    /// Axis-aligned bounds `(min, max)` in the source file's units, or `None`
    /// when the sub-object has no geometry (an empty mesh).
    pub bounds: Option<(Vec3, Vec3)>,
    /// How many materials this sub-object uses. A sub-mesh references at most one
    /// material in the engine's data model, so this is `0` or `1`.
    pub material_count: usize,
}

impl SubObjectEntry {
    /// The footprint extent `(max - min)`, or `Vec3::ZERO` when bounds are absent.
    /// A convenience for placement math (the diagonal of the AABB).
    pub fn size(&self) -> Vec3 {
        match self.bounds {
            Some((min, max)) => max - min,
            None => Vec3::ZERO,
        }
    }

    fn from_sub_mesh(path: &str, sub: &SubMesh) -> Self {
        let reference = AssetRef {
            path: path.to_string(),
            sub_object: sub.id.clone(),
        }
        .to_string_ref();
        Self {
            id: sub.id.clone(),
            reference,
            bounds: sub.bounds(),
            material_count: usize::from(sub.material.is_some()),
        }
    }
}

/// One source file under the asset root and the sub-objects it exposes.
#[derive(Clone, Debug, PartialEq)]
pub struct AssetEntry {
    /// The source file path (the same string the references embed), e.g.
    /// `project/models/crates.glb`.
    pub path: String,
    /// The file's addressable sub-objects, in file order.
    pub sub_objects: Vec<SubObjectEntry>,
    /// Number of materials in the file's shared material table.
    pub material_count: usize,
}

/// The whole project's importable assets: every model file and its sub-objects.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AssetManifest {
    pub assets: Vec<AssetEntry>,
}

impl AssetManifest {
    /// Total sub-object count across every file — the number of distinct things
    /// an agent could instantiate.
    pub fn sub_object_count(&self) -> usize {
        self.assets.iter().map(|a| a.sub_objects.len()).sum()
    }
}

/// Build the manifest for everything importable under `root`. Walks the tree for
/// model files ([`is_model_path`]), imports each, and records its sub-objects.
///
/// Files that fail to import (a corrupt or partial source) are skipped rather than
/// aborting the whole walk — a single bad file shouldn't blind the agent to the
/// rest of the project. Output is deterministic: paths are sorted, sub-objects keep
/// the importer's file order.
pub fn build_manifest(root: &Path) -> AssetManifest {
    let mut paths = Vec::new();
    collect_model_paths(root, &mut paths);
    paths.sort();

    let assets = paths
        .iter()
        .filter_map(|path| entry_for(path))
        .collect::<Vec<_>>();
    AssetManifest { assets }
}

/// Import one file into its [`AssetEntry`], or `None` if it can't be imported.
fn entry_for(path: &str) -> Option<AssetEntry> {
    let asset = import_file(Path::new(path)).ok()?;
    let sub_objects = asset
        .sub_meshes
        .iter()
        .map(|sub| SubObjectEntry::from_sub_mesh(path, sub))
        .collect();
    Some(AssetEntry {
        path: path.to_string(),
        sub_objects,
        material_count: asset.materials.len(),
    })
}

/// Recursively gather model-file paths under `dir` (forward-slashed). Unreadable
/// directories are silently skipped, mirroring the content browser's tolerance.
fn collect_model_paths(dir: &Path, out: &mut Vec<String>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_model_paths(&path, out);
        } else {
            let as_str = path.to_string_lossy().replace('\\', "/");
            if is_model_path(&as_str) {
                out.push(as_str);
            }
        }
    }
}
