//! One cache per thread, keyed by `key`: `poll` drains the write log once per physics
//! step (through `generation`) and inspector frame, never per lookup.
mod file;
#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use rusty_components::{ColliderComponent, PhysicsMaterial};
use rusty_core::fs::freshness::Freshness;
use rusty_core::fs::written::Stale;
use rusty_core::project;

pub use file::{parse, to_json};

pub const EXTENSION: &str = "physmat";

struct Cache {
    entries: BTreeMap<String, PhysicsMaterial>,
    freshness: Freshness,
    warned: BTreeSet<String>,
    project: u64,
    generation: u64,
}

impl Cache {
    fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            freshness: Freshness::new(),
            warned: BTreeSet::new(),
            project: project::moves(),
            generation: 0,
        }
    }

    fn follow_project(&mut self) {
        if self.project != project::moves() {
            let generation = self.generation + 1;
            *self = Self::new();
            self.generation = generation;
        }
    }

    fn refresh(&mut self) {
        self.follow_project();
        let changes = self.freshness.poll_with(|p| Path::new(p).exists());
        if changes.stale.is_empty() {
            return;
        }
        let stale = Stale::new(changes.stale);
        let before = self.entries.len();
        self.entries.retain(|path, _| !stale.matches(path));
        let missed = self.freshness.forget_misses_where(|p| stale.matches(p));
        if missed || self.entries.len() != before {
            self.generation += 1;
        }
    }

    fn lookup(&mut self, path: &str) -> Option<PhysicsMaterial> {
        self.follow_project();
        if let Some(m) = self.entries.get(path) {
            return Some(*m);
        }
        if self.freshness.is_miss(path) {
            return None;
        }
        match file::read(Path::new(path)) {
            Ok(m) => {
                self.entries.insert(path.to_string(), m);
                Some(m)
            }
            Err(why) => {
                self.freshness.miss(path);
                if self.warned.insert(path.to_string()) {
                    log::warn!("[Physics] physics material '{path}': {why}; using the default");
                }
                None
            }
        }
    }
}

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::new(Cache::new());
}

pub fn lookup(path: &str) -> Option<PhysicsMaterial> {
    let path = key(path);
    CACHE.with(|c| c.borrow_mut().lookup(&path))
}

pub fn poll() {
    CACHE.with(|c| c.borrow_mut().refresh());
}

/// Moves whenever an asset's values may have changed. Physics reads it only to decide
/// whether to re-apply every collider's material, so the sim never depends on it.
pub fn generation() -> u64 {
    CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        cache.refresh();
        cache.generation
    })
}

pub fn key(path: &str) -> String {
    rusty_core::paths::stored(path)
}

pub fn write(path: &str, material: PhysicsMaterial) -> Result<PhysicsMaterial, String> {
    let path = key(path);
    let material = file::sanitized(material)?;
    file::write(Path::new(&path), &material)?;
    CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        cache.refresh();
        cache.entries.insert(path.clone(), material);
        cache.warned.remove(&path);
        cache.generation += 1;
    });
    Ok(material)
}

pub fn effective(collider: &ColliderComponent) -> PhysicsMaterial {
    match &collider.material_asset {
        Some(path) => lookup(path).unwrap_or_default(),
        None => collider.material,
    }
}

pub fn named(collider: &ColliderComponent) -> Option<String> {
    let path = collider.material_asset.as_ref()?;
    lookup(path).map(|_| key(path))
}
