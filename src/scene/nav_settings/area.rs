//! src/scene/nav_settings/area.rs — the area table's data: rows, built-ins, limits (#460).
//!
//! What the per-scene area table (`NavMeshSettings::areas`) stores and the ids every
//! span, link and modifier volume carries. The search-time behaviour (cost clamping,
//! the cost table, agent masks) lives in `navigation::areas`, which reads this.

use serde::{Deserialize, Serialize};

/// How many areas a scene can define: one bit each in an `u32` mask.
pub const MAX_AREAS: usize = 32;
/// The built-in area every span has by default.
pub const WALKABLE_AREA: u8 = 0;
/// The built-in area that removes the spans a modifier volume covers.
pub const NOT_WALKABLE_AREA: u8 = 1;
/// The mask of an agent that may enter every area (Unity's "Everything").
pub const ALL_AREAS: u32 = u32::MAX;
/// The lowest cost an area may have; keeps the A\* heuristic admissible.
pub const MIN_AREA_COST: f32 = 1.0;

/// One row of the area table: its name and its default cost multiplier.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NavArea {
    pub name: String,
    pub cost: f32,
}

/// The built-in table: `Walkable` and `NotWalkable`, both cost 1.
pub fn default_areas() -> Vec<NavArea> {
    ["Walkable", "NotWalkable"]
        .map(|name| NavArea {
            name: name.to_string(),
            cost: MIN_AREA_COST,
        })
        .to_vec()
}
