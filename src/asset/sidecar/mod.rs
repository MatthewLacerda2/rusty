//! src/asset/sidecar/mod.rs — the `<file>.meta` import-settings sidecar.
//!
//! Unity-style: every imported source file gets a sibling `<file>.meta` holding
//! **import settings only** — never identity. Reference identity is path-based
//! (`path::sub_object`), DECIDED in issue #74, so the sidecar deliberately carries
//! no GUID. The sub-object map it stores is a human-readable cache of what the
//! file exposes (so the model inspector can list sub-objects without re-importing);
//! it is regenerated from the file on demand and is never the source of truth.
//!
//! Format is JSON for diffability, matching the scene document's choice.
//!
//! stored-format: the `<file>.meta` import-settings sidecar

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

mod collider;
mod texture;

pub use collider::{closes_loop, looped, MeshColliderKind, SubObjectCollider};

pub use texture::{
    fit_data_slot, fit_normal_slot, parse_max_size, set_texture, texture, Compression, FilterMode,
    TextureSettings, TextureType, WrapMode, DEFAULT_MAX_SIZE, MAX_SIZES,
};

use super::humanoid::HumanoidSettings;
use super::lightmap_uv::LightmapUvSettings;
use super::mesh_data::ImportedAsset;
use super::ImportError;
use crate::core::fs;

/// The `.meta` extension appended to the full source filename (Unity-style), so
/// `crates.glb` → `crates.glb.meta`.
const META_EXTENSION: &str = "meta";

/// Per-file import settings + a cached sub-object listing. Settings here are the
/// only authoritative thing in the sidecar; the sub-object map is a convenience
/// cache. `#[serde(default)]` throughout so a sparse, hand-written sidecar loads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportSettings {
    /// The model's *Scale Factor* (#1398): one uniform scale the importer applies
    /// to the vertices, the skeleton and the animation translations
    /// (`asset::scale_factor`). Default 1.
    #[serde(default = "default_scale")]
    pub scale: f32,
    /// Cached, human-readable list of the addressable sub-object ids in the file.
    #[serde(default)]
    pub sub_objects: Vec<String>,
    /// Per-sub-object mesh-collider choice (#77), keyed by sub-object id: its kind,
    /// and the sub-object whose geometry builds it (#1960, `collider`). A missing
    /// key means "no collider"; `BTreeMap` keeps the sidecar JSON stably ordered.
    #[serde(default, deserialize_with = "collider::read_map")]
    pub colliders: BTreeMap<String, SubObjectCollider>,
    /// Generate Lightmap UVs (#831): whether the importer unwraps a lightmap UV, and
    /// how. Off by default, as in Unity.
    #[serde(default)]
    pub lightmap_uvs: LightmapUvSettings,
    /// A texture's wrap, filter and mipmap settings (#1323); defaults for a model.
    #[serde(default)]
    pub texture: TextureSettings,
    /// The humanoid bone map and the models whose clips are retargeted onto
    /// this one (#1747). Left out of the file while empty.
    #[serde(default, skip_serializing_if = "HumanoidSettings::is_empty")]
    pub humanoid: HumanoidSettings,
}

fn default_scale() -> f32 {
    1.0
}

impl Default for ImportSettings {
    fn default() -> Self {
        Self {
            scale: default_scale(),
            sub_objects: Vec::new(),
            colliders: BTreeMap::new(),
            lightmap_uvs: LightmapUvSettings::default(),
            texture: TextureSettings::default(),
            humanoid: HumanoidSettings::default(),
        }
    }
}

impl ImportSettings {
    /// Refresh the cached sub-object listing from a freshly imported asset.
    pub fn with_sub_objects(mut self, asset: &ImportedAsset) -> Self {
        self.sub_objects = asset.sub_mesh_ids();
        self
    }
}

/// Set a model's Generate Lightmap UVs settings (#831), clamped to Unity's ranges,
/// keeping the rest of its sidecar. The one setter the model inspector and
/// `Assets.SetLightmapUVSettings` share; the next import of the file applies it.
pub fn set_lightmap_uvs(source: &Path, settings: LightmapUvSettings) -> Result<(), ImportError> {
    let mut stored = load(source)?;
    stored.lightmap_uvs = settings.clamped();
    save(source, &stored)
}

/// Set a model's Scale Factor (#1398), sanitized (`scale_factor::sanitize`),
/// keeping the rest of its sidecar. The one setter the model inspector and
/// `Assets.SetModelImportSettings` share; the next import of the file applies it.
pub fn set_scale(source: &Path, scale: f32) -> Result<(), ImportError> {
    let mut stored = load(source)?;
    stored.scale = super::scale_factor::sanitize(scale, stored.scale);
    save(source, &stored)
}

/// The sidecar path for a source file: `crates.glb` → `crates.glb.meta`.
pub fn meta_path(source: &Path) -> PathBuf {
    let mut name = source
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".");
    name.push(META_EXTENSION);
    source.with_file_name(name)
}

/// Load a source file's sidecar, or `Default` if none exists yet. Collision meshes
/// that name each other (a hand edit) read as their own, warned once per file.
pub fn load(source: &Path) -> Result<ImportSettings, ImportError> {
    let path = meta_path(source);
    // Windows can briefly deny the open while a concurrent save replaces the file.
    match fs::retry_transient(|| std::fs::read_to_string(&path)) {
        Ok(text) => {
            let mut settings: ImportSettings =
                serde_json::from_str(&text).map_err(|e| ImportError::Sidecar(e.to_string()))?;
            let looped = collider::break_loops(&mut settings.colliders);
            if !looped.is_empty() {
                collider::warn_loop(source, &looped);
            }
            Ok(settings)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ImportSettings::default()),
        Err(e) => Err(ImportError::Sidecar(e.to_string())),
    }
}

/// Write a source file's sidecar (pretty JSON, diffable).
///
/// The write is **atomic** ([`fs::write_atomic`]). A plain write truncates first, so
/// anything reading the same sidecar at that moment sees an empty or half-written
/// file and fails to parse it — and every caller here treats a sidecar error as
/// "asset unavailable", which degrades silently (a preview quietly showing a
/// fallback sphere, say). Two systems importing the same asset at once is ordinary,
/// so the reader must never observe a partial file.
pub fn save(source: &Path, settings: &ImportSettings) -> Result<(), ImportError> {
    let json =
        serde_json::to_string_pretty(settings).map_err(|e| ImportError::Sidecar(e.to_string()))?;
    fs::write_atomic(&meta_path(source), json.as_bytes())
        .map_err(|e| ImportError::Sidecar(e.to_string()))
}

/// Import the file, then write/refresh its sidecar's cached sub-object map,
/// preserving any existing settings; a sidecar already current is not re-written. Returns the file's shared import
/// ([`super::imported`], #857) for the caller.
pub fn import_and_sync_sidecar(source: &Path) -> Result<super::Imported, ImportError> {
    let asset = super::imported(source)?;
    let stored = load(source)?;
    let settings = stored.clone().with_sub_objects(&asset);
    // Unchanged, it is not written again: every instantiate syncs, and a write
    // is a change other caches follow (#857).
    if settings != stored || !meta_path(source).is_file() {
        save(source, &settings)?;
    }
    Ok(asset)
}
