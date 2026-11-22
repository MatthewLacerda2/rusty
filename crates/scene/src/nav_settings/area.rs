use serde::{Deserialize, Serialize};

use super::NavMeshSettings;

/// One bit each in a `u32` agent mask.
pub const MAX_AREAS: usize = 32;
pub const WALKABLE_AREA: u8 = 0;
pub const NOT_WALKABLE_AREA: u8 = 1;
pub const ALL_AREAS: u32 = u32::MAX;
/// The lowest cost multiplier, which keeps the A* heuristic admissible.
pub const MIN_AREA_COST: f32 = 1.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NavArea {
    pub name: String,
    pub cost: f32,
}

pub fn default_areas() -> Vec<NavArea> {
    ["Walkable", "NotWalkable"]
        .map(|name| NavArea {
            name: name.to_string(),
            cost: MIN_AREA_COST,
        })
        .to_vec()
}

pub fn clamp_cost(cost: f32) -> Option<f32> {
    cost.is_finite().then(|| cost.max(MIN_AREA_COST))
}

pub fn area_index(areas: &[NavArea], name: &str) -> Option<u8> {
    areas.iter().position(|a| a.name == name).map(|i| i as u8)
}

impl NavMeshSettings {
    pub fn set_area_cost(&mut self, name: &str, cost: f32) -> Option<u8> {
        let cost = clamp_cost(cost)?;
        let i = area_index(&self.areas, name)?;
        self.areas[i as usize].cost = cost;
        Some(i)
    }

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
