//! crates/navigation/src/areas.rs — navigation areas and their costs (#460).
//!
//! The table's data (rows, built-in ids, limits) is scene data in
//! `scene::nav_settings`; this module is what the search does with it.
//!
//! Unity's NavMesh areas: every walkable span (and every off-mesh link) carries an
//! **area id**, `0..MAX_AREAS`. The per-scene area table (`NavMeshSettings::areas`)
//! names each area and gives it a **cost multiplier**: A\* charges a step
//! `distance × cost`, so an agent prefers a cheap route and crosses an expensive
//! one only when no cheaper way exists. An agent's **area mask** (bit `i` = area
//! `i`) says which areas it may enter at all.
//!
//! * `Walkable` (0) is every span's area unless a `NavMeshModifier` on its object
//!   (#1959) or a `NavMeshModifierVolume` around it says otherwise (the volume wins);
//!   `NotWalkable` (1) removes the spans a volume covers, and leaves an object's tops
//!   unwalkable while it still blocks what is under it.
//! * Costs are clamped to at least 1, so the octile heuristic (one unit per cell)
//!   stays admissible and A\* stays optimal.
//! * Areas are assigned at bake time, but a cost is read at search time: changing a
//!   cost needs no rebake. The graph keeps a copy of the cost table
//!   ([`NavigationGraph::area_costs`]) that `sync` refreshes every tick, and a change
//!   invalidates every cached agent path so agents re-plan on the next tick.

pub use rusty_scene::nav_settings::{
    area_index, clamp_cost, default_areas, NavArea, ALL_AREAS, MAX_AREAS, MIN_AREA_COST,
    NOT_WALKABLE_AREA, WALKABLE_AREA,
};

use super::NavigationGraph;

/// An authored area id as the bake reads it: one past the table's limit (a
/// hand-edited scene file) is clamped to the last id, so it never indexes past it.
pub(super) fn clamp_area(area: u8) -> u8 {
    area.min((MAX_AREAS - 1) as u8)
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

/// Whether `mask` lets an agent into `area`.
pub fn in_mask(mask: u32, area: u8) -> bool {
    (area as usize) < MAX_AREAS && mask & (1 << area) != 0
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
