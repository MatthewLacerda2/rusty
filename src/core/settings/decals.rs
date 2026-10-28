//! src/core/settings/decals.rs — `DecalLimits`: the per-game decal budgets (#1517).
//!
//! How many decals a game keeps whole at once, in three buckets: per owner entity
//! (a bone is its own owner), the world (decals stamped with no owner) and the
//! scene's total, a performance ceiling. Past a budget the oldest decal of that
//! bucket fades out (`scene::decal::DecalSet`).
//!
//! The project's values live in the tracked `project.rusty` under `"decals"`
//! (`core::project::ProjectFile`) and are applied to the scene on entering Play;
//! `Decals.SetLimits` changes the running game's until the next Play.

use serde::{Deserialize, Serialize};

/// The decal budgets. Every field is `#[serde(default)]`, so a partial
/// `"decals"` block keeps the defaults for what it omits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DecalLimits {
    /// Whole decals one owner entity keeps.
    pub max_per_owner: u32,
    /// Whole decals with no owner (world space), as one bucket.
    pub max_world: u32,
    /// Whole decals in the scene, every bucket together.
    pub max_total: u32,
}

impl Default for DecalLimits {
    fn default() -> Self {
        Self {
            max_per_owner: 256,
            max_world: 1024,
            max_total: 4096,
        }
    }
}
