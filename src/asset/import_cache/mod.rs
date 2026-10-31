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
//! - **Fresh**: an entry records every file its parse read: the one parsed (the
//!   source, or for a converted `.fbx`/`.blend` its cached `.glb`, so a game
//!   exported without its sources still hits) and the others it pulled in
//!   ([`reads`]: an `.obj`'s `.mtl`, a `.gltf`'s `.bin`). Each lookup drains the
//!   write log (`core::fs::written`) through a [`Freshness`], as the renderer's
//!   texture cache does, and drops every entry whose source or read files were
//!   written; the sidecar is not followed, since its geometry settings are the
//!   key. Each file's size and modified time are kept too, so a file another
//!   process re-wrote with no watch running (a headless session, a test) is
//!   parsed again: modified time is metadata that decides only whether to read
//!   the file, never what the sim sees. A converted source has its conversion
//!   brought up to date first, once per lookup, so a re-conversion its
//!   dependencies forced is in the log before it is drained. Opening another
//!   project drops every entry (keys are project-relative).
//! - **Bounded**: least-recently-used entries are dropped once the cache holds
//!   more than [`BUDGET_BYTES`] of geometry (vertices, indices and animation
//!   keys, estimated by [`approx_bytes`]) beside its largest entry: one level
//!   file over the budget on its own stays while smaller files come and go, so
//!   it is still parsed once per load.
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

mod reads;
mod revision;
mod size;

use reads::{files_of, stamp, Files, Stamp};
pub use revision::MeshRevision;
pub use size::approx_bytes;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use super::lightmap_uv::LightmapUvSettings;
use super::mesh_data::ImportedAsset;
use super::{ImportError, ImportSettings};
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

struct Entry {
    geometry: Geometry,
    /// What the parse read, the parsed file first ([`files_of`]).
    files: Files,
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
    let key = path.to_string_lossy().into_owned();
    // The one conversion check: a re-conversion notes the source in the write
    // log, which the lookup below drains.
    let read = super::read_target(path).inspect_err(|_| {
        CACHE.with_borrow_mut(|cache| cache.remove(&key));
    })?;
    let settings = super::settings_of(path);
    let geometry = Geometry::of(&settings);
    let held = CACHE.with_borrow_mut(|cache| {
        cache.enter(project::moves());
        cache.forget_written();
        cache.hit(&key, geometry)
    });
    if let Some(import) = held {
        return Ok(import);
    }
    #[cfg(test)]
    PARSES.set(PARSES.get() + 1);
    // Stamped before the parse: a write during it is a change next time.
    let files = files_of(&read);
    let fresh = super::import_with(path, &read, &settings);
    CACHE.with_borrow_mut(|cache| match fresh {
        Ok(asset) => Ok(cache.insert(key, geometry, files, asset, BUDGET_BYTES)),
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

    /// Drop the entries whose source or read files the engine wrote since the
    /// last look.
    fn forget_written(&mut self) {
        let changes = self
            .freshness
            .get_or_insert_with(Freshness::new)
            .poll_with(|_| false);
        if changes.stale.is_empty() {
            return;
        }
        let stale = Stale::new(changes.stale);
        let gone: Vec<String> = (self.by_path.iter())
            .filter(|(p, e)| stale.matches(p) || e.files.iter().any(|(f, _)| stale.matches(f)))
            .map(|(p, _)| p.clone())
            .collect();
        for path in gone {
            self.remove(&path);
        }
    }

    /// The entry for `key` when its settings match and every file it read reads
    /// as it did (the parsed one still there).
    fn hit(&mut self, key: &str, geometry: Geometry) -> Option<Imported> {
        self.lookups += 1;
        let lookups = self.lookups;
        let entry = self.by_path.get_mut(key)?;
        let same = |(i, (file, was)): (usize, &(String, Stamp))| {
            let now = stamp(Path::new(file));
            now == *was && (i > 0 || now.is_some())
        };
        if entry.geometry != geometry || !entry.files.iter().enumerate().all(same) {
            return None;
        }
        entry.used = lookups;
        Some(entry.import.clone())
    }

    fn insert(
        &mut self,
        key: String,
        geometry: Geometry,
        files: Files,
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
            files,
            import: import.clone(),
            bytes,
            used,
        };
        self.by_path.insert(key.clone(), entry);
        self.evict_past(&key, budget);
        import
    }

    /// Drop least-recently-used entries other than `keep` until the cache holds
    /// no more than `budget` beside its largest entry, so a lone entry over the
    /// budget is never evicted to make room for a smaller one.
    fn evict_past(&mut self, keep: &str, budget: usize) {
        let largest = |c: &Self| c.by_path.values().map(|e| e.bytes).max().unwrap_or(0);
        while self.bytes > budget.saturating_add(largest(self)) {
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

#[cfg(test)]
mod budget_tests;
#[cfg(test)]
mod tests;
