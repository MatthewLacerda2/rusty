//! src/asset/sidecar.rs — the `<file>.meta` import-settings sidecar.
//!
//! Unity-style: every imported source file gets a sibling `<file>.meta` holding
//! **import settings only** — never identity. Reference identity is path-based
//! (`path::sub_object`), DECIDED in issue #74, so the sidecar deliberately carries
//! no GUID. The sub-object map it stores is a human-readable cache of what the
//! file exposes (so the Content Browser can list sub-objects without re-importing);
//! it is regenerated from the file on demand and is never the source of truth.
//!
//! Format is JSON for diffability, matching the scene document's choice.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::mesh_data::ImportedAsset;
use super::ImportError;

/// The `.meta` extension appended to the full source filename (Unity-style), so
/// `crates.glb` → `crates.glb.meta`.
pub const META_EXTENSION: &str = "meta";

/// How a sub-object's baked mesh collider is shaped (#77). `TriMesh` is an exact
/// triangle mesh (static geometry); `Convex` is a convex hull (cheaper, usable on
/// dynamic bodies). A sub-object absent from the sidecar map gets no collider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeshColliderKind {
    TriMesh,
    Convex,
}

impl MeshColliderKind {
    /// `true` when this kind bakes a convex hull rather than a triangle mesh.
    pub fn is_convex(self) -> bool {
        matches!(self, MeshColliderKind::Convex)
    }
}

/// Per-file import settings + a cached sub-object listing. Settings here are the
/// only authoritative thing in the sidecar; the sub-object map is a convenience
/// cache. `#[serde(default)]` throughout so older/hand-written sidecars still load.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportSettings {
    /// Uniform scale applied at instantiation time (authoring-unit fix-up).
    #[serde(default = "default_scale")]
    pub scale: f32,
    /// Whether to keep authored normals (vs. recomputing). Recompute is a later
    /// issue; this records the intent so the setting round-trips.
    #[serde(default = "default_true")]
    pub import_normals: bool,
    /// Cached, human-readable list of the addressable sub-object ids in the file.
    #[serde(default)]
    pub sub_objects: Vec<String>,
    /// Per-sub-object mesh-collider choice (#77), keyed by sub-object id. A missing
    /// key means "no collider"; `BTreeMap` keeps the sidecar JSON stably ordered.
    #[serde(default)]
    pub colliders: BTreeMap<String, MeshColliderKind>,
}

fn default_scale() -> f32 {
    1.0
}
fn default_true() -> bool {
    true
}

impl Default for ImportSettings {
    fn default() -> Self {
        Self {
            scale: default_scale(),
            import_normals: default_true(),
            sub_objects: Vec::new(),
            colliders: BTreeMap::new(),
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

/// Load a source file's sidecar, or `Default` if none exists yet.
pub fn load(source: &Path) -> Result<ImportSettings, ImportError> {
    let path = meta_path(source);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| ImportError::Sidecar(e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ImportSettings::default()),
        Err(e) => Err(ImportError::Sidecar(e.to_string())),
    }
}

/// Write a source file's sidecar (pretty JSON, diffable).
///
/// The write is **atomic**: the JSON goes to a temp file beside the target and is
/// then renamed over it. A plain write truncates first, so anything reading the same
/// sidecar at that moment sees an empty or half-written file and fails to parse it —
/// and every caller here treats a sidecar error as "asset unavailable", which
/// degrades silently (a preview quietly showing a fallback sphere, say). Two systems
/// importing the same asset at once is ordinary, so the reader must never observe a
/// partial file.
pub fn save(source: &Path, settings: &ImportSettings) -> Result<(), ImportError> {
    let path = meta_path(source);
    let json =
        serde_json::to_string_pretty(settings).map_err(|e| ImportError::Sidecar(e.to_string()))?;
    let temp = temp_path(&path);
    std::fs::write(&temp, json).map_err(|e| ImportError::Sidecar(e.to_string()))?;
    // Rename is atomic on every platform we target: a reader sees the whole old file
    // or the whole new one, never a torn one. On failure the temp file is cleaned up
    // so a crashed write leaves no litter beside the asset.
    std::fs::rename(&temp, &path).map_err(|e| {
        std::fs::remove_file(&temp).ok();
        ImportError::Sidecar(e.to_string())
    })
}

/// A scratch path beside `path`, unique per process and per call, so two concurrent
/// saves never write the same temp file (which would reintroduce the tear the
/// rename exists to prevent).
fn temp_path(path: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let ticket = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(format!(".{}.{ticket}.tmp", std::process::id()));
    path.with_file_name(name)
}

/// Import the file, then write/refresh its sidecar's cached sub-object map,
/// preserving any existing settings. Returns the imported asset for the caller.
pub fn import_and_sync_sidecar(source: &Path) -> Result<ImportedAsset, ImportError> {
    let asset = super::import_file(source)?;
    let settings = load(source)?.with_sub_objects(&asset);
    save(source, &settings)?;
    Ok(asset)
}
