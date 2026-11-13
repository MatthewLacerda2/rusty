//! crates/scene/src/nav_settings/area.rs — the area table's data: rows, built-ins, limits (#460).
//!
//! What the per-scene area table (`NavMeshSettings::areas`) stores, the ids every
//! span, link and modifier volume carries, and the edits that keep a row valid
//! (`define_area`, `set_area_cost`). The search-time behaviour (the cost table, agent
//! masks) lives in `navigation::areas`, which reads this.

use serde::{Deserialize, Serialize};

use super::NavMeshSettings;

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

/// A cost as the table stores it: at least [`MIN_AREA_COST`], `None` when not finite.
pub fn clamp_cost(cost: f32) -> Option<f32> {
    cost.is_finite().then(|| cost.max(MIN_AREA_COST))
}

/// The index of the area called `name` (case-sensitive, as Unity), if defined.
pub fn area_index(areas: &[NavArea], name: &str) -> Option<u8> {
    areas.iter().position(|a| a.name == name).map(|i| i as u8)
}

impl NavMeshSettings {
    /// Set the cost of the area called `name` (clamped to at least 1), returning its
    /// id, or `None` when no such area exists or the cost is not finite. The editor's
    /// area table and `Navigation.SetAreaCost` both write through here.
    pub fn set_area_cost(&mut self, name: &str, cost: f32) -> Option<u8> {
        let cost = clamp_cost(cost)?;
        let i = area_index(&self.areas, name)?;
        self.areas[i as usize].cost = cost;
        Some(i)
    }

    /// Define the area `name` with `cost`, or re-cost it when it exists. Returns its
    /// id, or `None` when the table is full, the name is empty or the cost not finite.
    pub fn define_area(&mut self, name: &str, cost: f32) -> Option<u8> {
        let clamped = clamp_cost(cost)?;
        if let Some(i) = self.set_area_cost(name, cost) {
            return Some(i);
        }
        if name.is_empty() || self.areas.len() >= MAX_AREAS {
            return None;
        }
        self.areas.push(NavArea {
            name: name.to_string(),
            cost: clamped,
        });
        Some((self.areas.len() - 1) as u8)
    }
}
