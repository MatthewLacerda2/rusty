//! Unity-style `<file>.meta`: import settings only, never identity (references are path-based, so
//! no GUID).
//! stored-format: the `<file>.meta` import-settings sidecar

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

mod collider;
mod texture;

pub use collider::{closes_loop, looped, MeshColliderKind, SubObjectCollider};

pub use texture::{
    check_bake_size, fit_data_slot, fit_normal_slot, fit_single_channel_slot, parse_max_size,
    prepare_bake, read_texture, set_texture, texture, Compression, FilterMode, TextureSettings,
    TextureType, WrapMode, DEFAULT_MAX_SIZE, MAX_SIZES,
};

use super::humanoid::HumanoidSettings;
use super::lightmap_uv::LightmapUvSettings;
use super::mesh_data::ImportedAsset;
use super::ImportError;
use rusty_core::fs;

const META_EXTENSION: &str = "meta";

/// `#[serde(default)]` so a sparse hand-written sidecar loads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportSettings {
    /// Uniform scale applied to vertices, skeleton and animation translations
    /// (`asset::scale_factor`). Default 1.
    #[serde(default = "default_scale")]
    pub scale: f32,
    /// Only a cache, regenerated from the import (`with_sub_objects`).
    #[serde(default)]
    pub sub_objects: Vec<String>,
    /// Keyed by sub-object id; a missing key means no collider.
    #[serde(default, deserialize_with = "collider::read_map")]
    pub colliders: BTreeMap<String, SubObjectCollider>,
    #[serde(default)]
    pub lightmap_uvs: LightmapUvSettings,
    #[serde(default)]
    pub texture: TextureSettings,
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
    pub fn with_sub_objects(mut self, asset: &ImportedAsset) -> Self {
        self.sub_objects = asset.sub_mesh_ids();
        self
    }
}

pub fn set_lightmap_uvs(source: &Path, settings: LightmapUvSettings) -> Result<(), ImportError> {
    let mut stored = load(source)?;
    stored.lightmap_uvs = settings.clamped();
    save(source, &stored)
}

pub fn set_scale(source: &Path, scale: f32) -> Result<(), ImportError> {
    let mut stored = load(source)?;
    stored.scale = super::scale_factor::sanitize(scale, stored.scale);
    save(source, &stored)
}

pub fn meta_path(source: &Path) -> PathBuf {
    let mut name = source
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".");
    name.push(META_EXTENSION);
    source.with_file_name(name)
}

pub fn load(source: &Path) -> Result<ImportSettings, ImportError> {
    Ok(stored(source)?.unwrap_or_default())
}

pub(crate) fn stored(source: &Path) -> Result<Option<ImportSettings>, ImportError> {
    let path = meta_path(source);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let mut settings: ImportSettings =
                serde_json::from_str(&text).map_err(|e| ImportError::Sidecar(e.to_string()))?;
            let looped = collider::break_loops(&mut settings.colliders);
            if !looped.is_empty() {
                collider::warn_loop(source, &looped);
            }
            Ok(Some(settings))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(ImportError::Sidecar(e.to_string())),
    }
}

/// Atomic ([`fs::write_atomic`]): a plain write truncates first, so a concurrent reader sees a
/// partial file and treats the asset as unavailable. Noted, so caches following a `.meta` drop
/// theirs.
pub fn save(source: &Path, settings: &ImportSettings) -> Result<(), ImportError> {
    let json =
        serde_json::to_string_pretty(settings).map_err(|e| ImportError::Sidecar(e.to_string()))?;
    fs::written::write(&meta_path(source), json.as_bytes())
        .map_err(|e| ImportError::Sidecar(e.to_string()))
}

pub fn import_and_sync_sidecar(source: &Path) -> Result<super::Imported, ImportError> {
    let asset = super::imported(source)?;
    let stored = load(source)?;
    let settings = stored.clone().with_sub_objects(&asset);
    // Unchanged: not rewritten, since a write is a change other caches follow.
    if settings != stored || !meta_path(source).is_file() {
        save(source, &settings)?;
    }
    Ok(asset)
}
