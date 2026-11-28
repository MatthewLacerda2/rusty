//! Layered navigation: an XZ grid whose cells each hold a list of walkable spans (stacked floors, a
//! bridge over a road), baked from the static colliders' triangles, with A\* over spans.

// Bare `.unwrap()` is denied here; test code is exempt via clippy.toml.
#![deny(clippy::unwrap_used)]

mod agents;
mod areas;
mod astar;
#[cfg(test)]
mod astar_tests;
mod avoidance;
mod bake;
mod bounds;
#[cfg(test)]
mod bounds_tests;
mod flood;
mod grid;
mod links;
mod object_modifier;
mod obstacle;
mod offmesh;
mod origin;
mod path;
mod pool;
#[cfg(test)]
mod probe_site;
mod reach;
mod request;
mod snap;
mod surface;
#[doc(hidden)]
pub mod test_support;
#[cfg(test)]
mod walkable_tests;

pub use agents::complete_off_mesh_link;
pub use agents::state::{
    is_at_target, remaining_corners, remaining_distance, reset_path, WARP_SNAP_DISTANCE,
};
pub use areas::{
    area_index, cost_table, default_areas, in_mask, NavArea, ALL_AREAS, MAX_AREAS, MIN_AREA_COST,
    NOT_WALKABLE_AREA, WALKABLE_AREA,
};
pub use astar::MAX_SEARCH_NODES;
pub use bake::{baked_fingerprint, CellRect, Rebake, Start};
pub use bounds::{bake_collider_ids, NavBounds};
pub use flood::{Flood, FloodSearch};
pub use grid::{NavSpan, NavigationGraph, SpanRef, DEFAULT_MAX_SLOPE, DEFAULT_MAX_STEP};
pub use obstacle::{tick_obstacles, ObstacleVolume};
pub use offmesh::{LinkEnd, OffMeshLink, OffMeshLinkData, OffMeshLinkKind};
pub use path::{path_length, NavPath, NavPathStatus, NavRaycastHit};
pub use request::islands::UNREACHABLE_SEARCH_NODES;
pub use request::DEFAULT_ITERATIONS_PER_FRAME;
pub use rusty_scene::nav_settings::{NavMeshSettings, DEFAULT_AGENT_HEIGHT};
pub use snap::SNAP_RINGS;
pub use surface::Surface;
