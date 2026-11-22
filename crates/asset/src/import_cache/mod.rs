//! Each model is parsed once, as Unity keeps an imported model in memory. Size and
//! modified time catch another process's writes (never read by the sim). The lock
//! is never held across a parse, so two threads may parse one file into the same value.
//! stored-format: the files the import cache writes beside a model (the hull sidecars)

mod counts;
pub mod hulls;
mod reads;
mod revision;
mod size;

use counts::{count_lookup, count_parse};
#[doc(hidden)]
pub use counts::{forget, lookups, parses};
use reads::{files_of, stamp, Files, Stamp};
pub(crate) use reads::{glb_json, parsed};
pub use revision::MeshRevision;
pub use size::approx_bytes;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex, PoisonError};

use super::humanoid::HumanoidSettings;
use super::lightmap_uv::LightmapUvSettings;
use super::mesh_data::ImportedAsset;
use super::{ImportError, ImportSettings};
use rusty_core::fs::freshness::Freshness;
use rusty_core::fs::written::Stale;
use rusty_core::project;

/// Fits a Dust2-sized level's whole model set.
pub const BUDGET_BYTES: usize = 512 << 20;

#[derive(Clone, Debug)]
pub struct Imported {
    pub asset: Arc<ImportedAsset>,
    pub revision: MeshRevision,
}

impl Imported {
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

#[derive(Clone, Debug, PartialEq)]
struct Geometry {
    scale: u32,
    lightmap_uvs: LightmapUvSettings,
    humanoid: HumanoidSettings,
}

impl Geometry {
    fn of(settings: &ImportSettings) -> Self {
        Self {
            scale: settings.scale.to_bits(),
            lightmap_uvs: settings.lightmap_uvs,
            humanoid: settings.humanoid.clone(),
        }
    }
}

struct Entry {
    geometry: Geometry,
    /// The parsed file first.
    files: Files,
    import: Imported,
    bytes: usize,
    used: u64,
}

#[derive(Default)]
struct Cache {
    freshness: Option<Freshness>,
    project: Option<u64>,
    by_path: BTreeMap<String, Entry>,
    lookups: u64,
    bytes: usize,
}

static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(Mutex::default);

fn with_cache<T>(f: impl FnOnce(&mut Cache) -> T) -> T {
    f(&mut CACHE.lock().unwrap_or_else(PoisonError::into_inner))
}

pub fn imported(path: &Path) -> Result<Imported, ImportError> {
    count_lookup();
    let key = path.to_string_lossy().into_owned();
    let read = super::read_target(path).inspect_err(|_| {
        with_cache(|cache| cache.remove(&key));
    })?;
    let settings = super::settings_of(path);
    for from in &settings.humanoid.clips_from {
        let _ = super::read_target(Path::new(from));
    }
    let geometry = Geometry::of(&settings);
    let held = with_cache(|cache| {
        cache.enter(project::moves());
        cache.forget_written();
        cache.hit(&key, &geometry)
    });
    if let Some(import) = held {
        return Ok(import);
    }
    count_parse();
    // Stamped before the parse: a write during it is a change next time.
    let files = files_of(&read, &settings.humanoid.clips_from);
    let fresh = super::import_with(path, &read, &settings).inspect(|a| hulls::load(path, a));
    with_cache(|cache| match fresh {
        Ok(asset) => Ok(cache.insert(key, geometry, files, asset, BUDGET_BYTES)),
        Err(e) => {
            cache.remove(&key);
            Err(e)
        }
    })
}

impl Cache {
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

    fn hit(&mut self, key: &str, geometry: &Geometry) -> Option<Imported> {
        self.lookups += 1;
        let lookups = self.lookups;
        let entry = self.by_path.get_mut(key)?;
        let same = |(i, (file, was)): (usize, &(String, Stamp))| {
            let now = stamp(Path::new(file));
            now == *was && (i > 0 || now.is_some())
        };
        if entry.geometry != *geometry || !entry.files.iter().enumerate().all(same) {
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

    /// A lone entry over budget is never evicted for a smaller one.
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
mod borrow_tests;
#[cfg(test)]
mod budget_tests;
#[cfg(test)]
mod tests;
