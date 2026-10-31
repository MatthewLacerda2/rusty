//! src/asset/import_cache/ — each model file parsed once, not once per instance
//! (#857).
//!
//! A scene load re-imports every `"Asset"` mesh from its path-based reference
//! (#74), and so do a prefab stamp, a propagation, `Scene.Instantiate` and the
//! edit-mode follower of re-written models (`scene::rehydrate::reimport_written`).
//! Without a cache a level glb with 40 instanced sub-objects was parsed 40 times
//! per load, and with Generate Lightmap UVs ticked (#831) each parse unwrapped it
//! again. [`imported`] keeps the last import of each file and hands it back, as
//! Unity keeps an imported model in memory rather than reading the source per
//! instance.
//!
//! - **Key**: the path as written, plus the import settings that change geometry
//!   (the Scale Factor, #1398, and Generate Lightmap UVs). The sidecar is read on
//!   every lookup (one small file), so ticking a box is a different key and never
//!   serves the old import. One entry per path: a new key replaces the old one.
//! - **Fresh**: each lookup drains the write log (`core::fs::written`) through a
//!   [`Freshness`], as the renderer's texture cache does, and drops every entry
//!   whose file or `.meta` sidecar was written. The source's size and modified
//!   time are kept too, so a file another process re-wrote with no watch running
//!   (a headless session, a test) is parsed again: modified time is metadata that
//!   decides only whether to read the file, never what the sim sees. A converted
//!   source (`.fbx`, `.blend`) has its conversion brought up to date first, so a
//!   re-conversion its dependencies forced is in the log before it is drained.
//!   Opening another project drops every entry (keys are project-relative).
//! - **Bounded**: entries are dropped least-recently-used once their geometry
//!   passes [`BUDGET_BYTES`] (vertices, indices and animation keys, estimated by
//!   [`approx_bytes`]); the newest entry is always kept, however large, so one
//!   huge level file is still parsed once per load.
//! - **Revision**: each import takes one [`MeshRevision`], which every instance
//!   built from it shares, so the renderer's mesh slot for a reference uploads
//!   once; only a real re-import takes a new one.
//! - **Deterministic**: a cached import is the same value a fresh one is, a pure
//!   function of (file, settings); the cache only decides whether to parse.
//! - **A failed import is not kept**: the next lookup tries again, as before.
//!
//! One cache per thread, like the resolved texture references
//! (`asset::convert::texture_ref::resolved`): a bake or preview thread keeps its
//! own, and nothing takes a lock across a parse.

mod revision;

pub use revision::MeshRevision;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use super::lightmap_uv::LightmapUvSettings;
use super::mesh_data::ImportedAsset;
use super::{convert, sidecar, ImportError, ImportSettings};
use crate::core::fs::freshness::Freshness;
use crate::core::fs::written::Stale;
use crate::core::project;

/// How much geometry one thread's cache holds before it drops the least recently
/// used entries: a Dust2-sized level's whole model set fits (a few million
/// vertices), and a long editor session that opens many models stays under it.
pub const BUDGET_BYTES: usize = 512 << 20;

/// One import of a model file, shared by every instance built from it.
#[derive(Clone, Debug)]
pub struct Imported {
    pub asset: Arc<ImportedAsset>,
    /// The geometry revision every mesh built from this import carries.
    pub revision: MeshRevision,
}

impl Imported {
    /// An import nobody cached (a test's hand-built asset): a fresh revision.
    pub fn new(asset: ImportedAsset) -> Self {
        Self {
            asset: Arc::new(asset),
            revision: MeshRevision::next(),
        }
    }
}

impl std::ops::Deref for Imported {
    type Target = ImportedAsset;
    fn deref(&self) -> &ImportedAsset {
        &self.asset
    }
}

/// The import settings that change geometry: what an entry is keyed by beside
/// its path.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Geometry {
    scale: u32,
    lightmap_uvs: LightmapUvSettings,
}

impl Geometry {
    fn of(settings: &ImportSettings) -> Self {
        Self {
            scale: settings.scale.to_bits(),
            lightmap_uvs: settings.lightmap_uvs,
        }
    }
}

/// A file's size and modified time (`None`: it could not be read).
type Stamp = Option<(u64, Option<Duration>)>;

fn stamp(path: &Path) -> Stamp {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok();
    let since = modified.and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok());
    Some((meta.len(), since))
}

struct Entry {
    geometry: Geometry,
    stamp: Stamp,
    import: Imported,
    bytes: usize,
    /// The lookup count when it was last served (least recently used goes first).
    used: u64,
}

#[derive(Default)]
struct Cache {
    freshness: Option<Freshness>,
    /// The `project::moves` count the entries were read under.
    project: Option<u64>,
    by_path: BTreeMap<String, Entry>,
    lookups: u64,
    bytes: usize,
}

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::new(Cache::default());
}

#[cfg(test)]
thread_local! {
    static PARSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many files this thread's cache has parsed (tests).
#[cfg(test)]
pub fn parses() -> usize {
    PARSES.get()
}

/// The import of the model file at `path` with its sidecar's settings: the cached
/// one when the file, its settings and the write log say it is current, a fresh
/// [`super::import_file`] otherwise.
pub fn imported(path: &Path) -> Result<Imported, ImportError> {
    if convert::converter_for(path).is_some() {
        // A re-conversion notes the source in the write log; let it, then drain.
        let _ = convert::cached_gltf(path);
    }
    let settings = super::settings_of(path);
    let key = path.to_string_lossy().into_owned();
    let stamp = stamp(path);
    let geometry = Geometry::of(&settings);
    let held = CACHE.with_borrow_mut(|cache| {
        cache.enter(project::moves());
        cache.forget_written();
        cache.hit(&key, geometry, stamp)
    });
    if let Some(import) = held {
        return Ok(import);
    }
    #[cfg(test)]
    PARSES.set(PARSES.get() + 1);
    let fresh = super::import_with(path, &settings);
    CACHE.with_borrow_mut(|cache| match fresh {
        Ok(asset) => Ok(cache.insert(key, geometry, stamp, asset, BUDGET_BYTES)),
        Err(e) => {
            cache.remove(&key);
            Err(e)
        }
    })
}

impl Cache {
    /// Follow the open project: another one drops every entry.
    fn enter(&mut self, moves: u64) {
        if self.project != Some(moves) {
            self.clear();
            self.project = Some(moves);
        }
    }

    fn clear(&mut self) {
        self.by_path.clear();
        self.bytes = 0;
    }

    /// Drop the entries whose file or sidecar the engine wrote since the last look.
    fn forget_written(&mut self) {
        let changes = self
            .freshness
            .get_or_insert_with(Freshness::new)
            .poll_with(|_| false);
        if changes.stale.is_empty() {
            return;
        }
        let stale = Stale::new(changes.stale);
        let gone: Vec<String> = (self.by_path.keys())
            .filter(|p| {
                let meta = sidecar::meta_path(Path::new(p));
                stale.matches(p) || stale.matches(&meta.to_string_lossy())
            })
            .cloned()
            .collect();
        for path in gone {
            self.remove(&path);
        }
    }

    fn hit(&mut self, key: &str, geometry: Geometry, stamp: Stamp) -> Option<Imported> {
        self.lookups += 1;
        let lookups = self.lookups;
        let entry = self.by_path.get_mut(key)?;
        if entry.geometry != geometry || entry.stamp != stamp || stamp.is_none() {
            return None;
        }
        entry.used = lookups;
        Some(entry.import.clone())
    }

    fn insert(
        &mut self,
        key: String,
        geometry: Geometry,
        stamp: Stamp,
        asset: ImportedAsset,
        budget: usize,
    ) -> Imported {
        self.remove(&key);
        let bytes = approx_bytes(&asset);
        let import = Imported::new(asset);
        self.bytes += bytes;
        let used = self.lookups;
        let entry = Entry {
            geometry,
            stamp,
            import: import.clone(),
            bytes,
            used,
        };
        self.by_path.insert(key.clone(), entry);
        self.evict_past(&key, budget);
        import
    }

    /// Drop least-recently-used entries other than `keep` until under `budget`.
    fn evict_past(&mut self, keep: &str, budget: usize) {
        while self.bytes > budget {
            let oldest = (self.by_path.iter())
                .filter(|(p, _)| p.as_str() != keep)
                .min_by_key(|(_, e)| e.used)
                .map(|(p, _)| p.clone());
            let Some(oldest) = oldest else { return };
            self.remove(&oldest);
        }
    }

    fn remove(&mut self, key: &str) {
        if let Some(entry) = self.by_path.remove(key) {
            self.bytes -= entry.bytes;
        }
    }
}

/// About how much memory an import's geometry and animation keys take.
pub fn approx_bytes(asset: &ImportedAsset) -> usize {
    use std::mem::size_of;
    let keys = |c: &super::AnimationClip| -> usize {
        let t = c.tracks.iter();
        t.map(|t| {
            let times = t.translation.times.len() + t.rotation.times.len() + t.scale.times.len();
            times * size_of::<f32>()
                + (t.translation.values.len() + t.scale.values.len()) * size_of::<glam::Vec3>()
                + t.rotation.values.len() * size_of::<glam::Quat>()
        })
        .sum()
    };
    let subs = asset.sub_meshes.iter().map(|s| {
        s.vertices.len() * size_of::<super::MeshVertex>()
            + s.indices.len() * size_of::<u32>()
            + s.clips.iter().map(keys).sum::<usize>()
    });
    subs.sum()
}

#[cfg(test)]
mod tests;
