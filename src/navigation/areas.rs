//! src/navigation/areas.rs — navigation areas and their costs (#460).
//!
//! Unity's NavMesh areas: every walkable span (and every off-mesh link) carries an
//! **area id**, `0..MAX_AREAS`. The per-scene area table (`NavMeshSettings::areas`)
//! names each area and gives it a **cost multiplier**: A\* charges a step
//! `distance × cost`, so an agent prefers a cheap route and crosses an expensive
//! one only when no cheaper way exists. An agent's **area mask** (bit `i` = area
//! `i`) says which areas it may enter at all.
//!
//! * `Walkable` (0) is every span's area unless a `NavMeshModifierVolume` says
//!   otherwise; `NotWalkable` (1) removes the spans a volume covers.
//! * Costs are clamped to at least 1, so the octile heuristic (one unit per cell)
//!   stays admissible and A\* stays optimal.
//! * Areas are assigned at bake time, but a cost is read at search time: changing a
//!   cost needs no rebake. The graph keeps a copy of the cost table
//!   ([`NavigationGraph::area_costs`]) that `sync` refreshes every tick, and a change
//!   invalidates every cached agent path so agents re-plan on the next tick.

use serde::{Deserialize, Serialize};

use super::{NavMeshSettings, NavigationGraph};

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

/// The cost of every area id, from a table: rows past the table (and a non-finite
/// cost) read as [`MIN_AREA_COST`].
pub fn cost_table(areas: &[NavArea]) -> [f32; MAX_AREAS] {
    let mut costs = [MIN_AREA_COST; MAX_AREAS];
    for (c, a) in costs.iter_mut().zip(areas) {
        *c = clamp_cost(a.cost).unwrap_or(MIN_AREA_COST);
    }
    costs
}

/// The index of the area called `name` (case-sensitive, as Unity), if defined.
pub fn area_index(areas: &[NavArea], name: &str) -> Option<u8> {
    areas.iter().position(|a| a.name == name).map(|i| i as u8)
}

/// Whether `mask` lets an agent into `area`.
pub fn in_mask(mask: u32, area: u8) -> bool {
    (area as usize) < MAX_AREAS && mask & (1 << area) != 0
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

impl NavigationGraph {
    /// The cost multiplier of `area`.
    pub fn area_cost(&self, area: u8) -> f32 {
        self.area_costs
            .get(area as usize)
            .copied()
            .unwrap_or(MIN_AREA_COST)
    }

    /// Adopt the cost table of `areas`. A change takes effect on the next search and
    /// invalidates every cached agent path (agents re-plan on their next tick); no
    /// span is rebaked. Returns whether any cost changed.
    pub fn set_area_costs(&mut self, areas: &[NavArea]) -> bool {
        let costs = cost_table(areas);
        if costs == self.area_costs {
            return false;
        }
        self.area_costs = costs;
        self.invalidate_paths();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn costs_clamp_to_one_and_unknown_areas_cost_one() {
        let mut areas = default_areas();
        areas.push(NavArea {
            name: "Fire".into(),
            cost: 0.25,
        });
        areas.push(NavArea {
            name: "Mud".into(),
            cost: 4.0,
        });
        let t = cost_table(&areas);
        assert_eq!((t[0], t[2], t[3], t[31]), (1.0, 1.0, 4.0, 1.0));
        assert_eq!(area_index(&areas, "Mud"), Some(3));
        assert_eq!(area_index(&areas, "mud"), None, "names are case-sensitive");
        assert_eq!(clamp_cost(f32::NAN), None);
    }

    #[test]
    fn defining_areas_appends_rows_up_to_the_limit() {
        let mut s = NavMeshSettings::default();
        assert_eq!(s.define_area("Fire", 10.0), Some(2));
        assert_eq!(s.define_area("Fire", 0.5), Some(2), "redefining re-costs");
        assert_eq!(s.areas[2].cost, 1.0, "costs clamp to 1");
        assert_eq!(s.set_area_cost("Smoke", 2.0), None, "unknown area");
        assert_eq!(s.define_area("", 2.0), None);
        assert_eq!(s.define_area("Ice", f32::INFINITY), None);
        for i in s.areas.len()..MAX_AREAS {
            assert!(s.define_area(&format!("A{i}"), 1.0).is_some());
        }
        assert_eq!(s.define_area("Overflow", 1.0), None, "32 areas at most");
    }

    #[test]
    fn masks_test_one_bit_per_area() {
        assert!(in_mask(ALL_AREAS, 31));
        assert!(in_mask(0b101, 2) && !in_mask(0b101, 1));
        assert!(!in_mask(ALL_AREAS, 32), "past the table is never allowed");
    }

    #[test]
    fn a_cost_change_invalidates_paths_without_a_rebake() {
        let mut g = NavigationGraph::new(0.0, 4.0, 0.0, 4.0, 1.0);
        let generation = g.bake_generation;
        assert!(!g.set_area_costs(&default_areas()), "same costs: no change");
        let mut areas = default_areas();
        areas[0].cost = 3.0;
        assert!(g.set_area_costs(&areas));
        assert_eq!(g.area_cost(0), 3.0);
        assert_ne!(g.bake_generation, generation, "cached paths go stale");
        assert_eq!(
            g.changes_since(generation),
            None,
            "anything may have changed"
        );
    }
}
