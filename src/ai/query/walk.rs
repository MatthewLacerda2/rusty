//! Reachability and path length (#1892, decision 3): one bounded flood from the
//! querier answers every item, instead of an A\* per item.

use glam::Vec3;

use super::spec::Anchor;
use super::QueryWorld;
use crate::navigation::ALL_AREAS;

/// How far past the farthest item the flood may wander, as a factor of that
/// straight-line distance: a path around a wall or up a stair is longer than the
/// crow flies, but rarely twice as long.
const SLACK: f32 = 2.0;
/// And at least this much, metres, so an item next to the querier behind a short
/// wall is still found.
const MIN_REACH: f32 = 4.0;

/// Each item's travel cost from the querier along the navmesh, metres, or `None`
/// for an item it cannot reach (or one past the flood's bound). Walks the
/// querier's own areas when it is an agent, every area otherwise.
pub fn lengths(world: &QueryWorld, querier: Anchor, items: &[Vec3]) -> Vec<Option<f32>> {
    if items.is_empty() {
        return Vec::new();
    }
    let mask = querier
        .entity
        .and_then(|id| world.scene.world.nav_agent(id).map(|a| a.area_mask))
        .unwrap_or(ALL_AREAS);
    let from = querier.position;
    let farthest = items.iter().map(|p| p.distance(from)).fold(0.0, f32::max);
    let bound = (farthest * SLACK).max(farthest + MIN_REACH);
    let Some(flood) = world.nav.flood_to(from, items, bound, mask) else {
        return vec![None; items.len()];
    };
    items
        .iter()
        .map(|&p| flood.length_to(world.nav, p))
        .collect()
}
