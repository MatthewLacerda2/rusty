//! src/scene/physics_material/ — physics material assets (#1878), Unity's
//! `PhysicMaterial` asset.
//!
//! A collider's friction, bounciness and combine modes are stored inline on it
//! (#447), or come from a shared asset: a small JSON file in the project (by
//! convention under `assets/materials/`, extension [`EXTENSION`]) that the collider
//! names by its project path (`ColliderComponent::material_asset`). Editing the
//! asset retunes every collider that names it, in edit mode and during Play, and
//! every ray hit and collision contact names the asset it struck ([`named`]): the
//! key a game's own tables (penetration power, impact effects, footsteps) are read
//! with. The engine stores no penetration value: that is game data.
//!
//! - **One cache per thread**, keyed by the path as written, like the import cache
//!   (`asset::import_cache`): each asset file is read once. Every lookup drains the
//!   write log (`core::fs::written`) through a [`Freshness`], so a file the engine
//!   wrote ([`write`]) or the editor's file watch saw change outside it is read
//!   again, and opening another project drops every entry (keys are relative).
//! - **A missing or unreadable file is a miss**, warned of once per path: the
//!   collider takes the default material (Unity drops a missing reference to the
//!   default) and its hits name no material. The miss is retried on the
//!   freshness beat, so a file that appears later is picked up.
//! - **[`generation`]** moves whenever an asset's values may have changed: physics
//!   compares it with the one it last synced against and, when it moved,
//!   re-applies the material of every built collider (a re-sync, never a rebuild).
//!   It only decides whether to compare, so the sim never depends on it.

mod file;
#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::components::{ColliderComponent, PhysicsMaterial};
use crate::core::fs::freshness::Freshness;
use crate::core::fs::written::Stale;
use crate::core::project;

pub use file::{parse, to_json};

/// A physics material asset's file extension (Unity's `.physicMaterial`).
pub const EXTENSION: &str = "physmat";

/// One thread's loaded assets.
struct Cache {
    entries: BTreeMap<String, PhysicsMaterial>,
    freshness: Freshness,
    /// The paths already warned of, so a miss is reported once.
    warned: BTreeSet<String>,
    /// `project::moves()` when the entries were read.
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

    /// Drop what another project or a write made stale.
    fn refresh(&mut self) {
        if self.project != project::moves() {
            let generation = self.generation + 1;
            *self = Self::new();
            self.generation = generation;
        }
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
        self.refresh();
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

/// The asset at project path `path`, or `None` when it is missing or unreadable
/// (warned of once).
pub fn lookup(path: &str) -> Option<PhysicsMaterial> {
    CACHE.with(|c| c.borrow_mut().lookup(path))
}

/// A counter that moves whenever an asset's values may have changed: an asset
/// written, re-read or dropped, or another project opened.
pub fn generation() -> u64 {
    CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        cache.refresh();
        cache.generation
    })
}

/// Create or replace the asset at `path` with `material` (clamped into range), and
/// make every collider that names it use the new values from its next lookup.
/// `path` must end in `.physmat`; its folder is created when absent.
pub fn write(path: &str, material: PhysicsMaterial) -> Result<PhysicsMaterial, String> {
    let material = file::sanitized(material)?;
    file::write(Path::new(path), &material)?;
    CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        cache.refresh();
        cache.entries.insert(path.to_string(), material);
        cache.warned.remove(path);
        cache.generation += 1;
    });
    Ok(material)
}

/// The material `collider` puts on the solver: its asset's values, the default
/// when its asset no longer loads, or its inline values when it names none.
pub fn effective(collider: &ColliderComponent) -> PhysicsMaterial {
    match &collider.material_asset {
        Some(path) => lookup(path).unwrap_or_default(),
        None => collider.material,
    }
}

/// The material a hit on `collider` names (Unity's `hit.collider.sharedMaterial`):
/// its asset's path while that asset loads, else `None` (an inline or default
/// material, or a dangling path).
pub fn named(collider: &ColliderComponent) -> Option<String> {
    let path = collider.material_asset.as_ref()?;
    lookup(path).map(|_| path.clone())
}
