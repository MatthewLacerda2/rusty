//! The "see what I can place" half of authoring: each sub-object's footprint
//! (AABB) lets an agent pick "a sedan" and know its size.

use std::path::Path;

use glam::Vec3;

use rusty_core::paths;

use super::{import_file, is_model_path, AssetRef, ImportError, SubMesh};

#[derive(Clone, Debug, PartialEq)]
pub struct SubObjectEntry {
    /// The part after `::`, e.g. `Sedan`.
    pub id: String,
    pub reference: String,
    /// `None` for an empty mesh.
    pub bounds: Option<(Vec3, Vec3)>,
    /// Distinct materials its runs draw with; `0` when it names none.
    pub material_count: usize,
}

impl SubObjectEntry {
    /// `max - min`, or `Vec3::ZERO` when bounds are absent.
    pub fn size(&self) -> Vec3 {
        match self.bounds {
            Some((min, max)) => max - min,
            None => Vec3::ZERO,
        }
    }

    fn from_sub_mesh(path: &str, sub: &SubMesh) -> Result<Self, ImportError> {
        let reference = AssetRef::new(path, &sub.id)?.to_string_ref();
        Ok(Self {
            id: sub.id.clone(),
            reference,
            bounds: sub.bounds(),
            material_count: distinct_materials(sub),
        })
    }
}

fn distinct_materials(sub: &SubMesh) -> usize {
    let mut seen: Vec<usize> = sub.ranges.iter().filter_map(|r| r.material).collect();
    seen.sort_unstable();
    seen.dedup();
    seen.len()
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssetEntry {
    pub path: String,
    pub sub_objects: Vec<SubObjectEntry>,
    pub material_count: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AssetManifest {
    pub assets: Vec<AssetEntry>,
}

impl AssetManifest {
    #[cfg(test)]
    pub fn sub_object_count(&self) -> usize {
        self.assets.iter().map(|a| a.sub_objects.len()).sum()
    }
}

/// Files that fail to import are skipped. Paths are sorted.
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

fn entry_for(path: &str) -> Option<AssetEntry> {
    // A plain import: listing the project must not fill the import cache.
    let asset = import_file(Path::new(path)).ok()?;
    let sub_objects = asset
        .sub_meshes
        .iter()
        .map(|sub| SubObjectEntry::from_sub_mesh(path, sub))
        .collect::<Result<_, _>>()
        .inspect_err(|e| log::warn!("[Asset] manifest skips {path}: {e}"))
        .ok()?;
    Some(AssetEntry {
        path: path.to_string(),
        sub_objects,
        material_count: asset.materials.len(),
    })
}

/// Forward-slashed.
fn collect_model_paths(dir: &Path, out: &mut Vec<String>) {
    walk(dir, &mut |path| {
        let as_str = paths::to_slash(path);
        if is_model_path(&as_str) {
            out.push(as_str);
        }
    });
}

/// Leaves the import cache out (the source is the asset). A symlinked folder is
/// not followed, so a link back up the tree never loops.
pub fn walk(dir: &Path, visit: &mut dyn FnMut(&Path)) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if super::convert::is_import_cache(&path) {
            continue;
        }
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => walk(&path, visit),
            Ok(_) => visit(&path),
            Err(_) => {}
        }
    }
}
