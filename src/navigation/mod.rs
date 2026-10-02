//! Layered navigation (#454): an XZ grid whose cells each hold a list of walkable
//! spans (stacked floors, a bridge over a road), baked from the static colliders'
//! real triangles, with A\* over spans and the per-frame agent steering tick. Split
//! into focused submodules — `grid` (the `NavigationGraph` model and its compact span
//! storage), `bounds` (the XZ area the grid covers, resolved from the scene, #452),
//! `bake` (rasterise → spans → agent-radius erosion), `links` (which span a move
//! reaches), `snap` (which span a world point means), `astar` (the span search),
//! `offmesh` (drop, jump and authored links between spans, #462),
//! `path` (smoothed paths, `SamplePosition`, `Raycast`), `agents` (cached-path planning + the steering tick), and
//! `avoidance` (ORCA local avoidance between agents, #463) — all hanging off the
//! single re-exported [`NavigationGraph`] type.

// Panic-free sim core (#195): bare `.unwrap()` is denied here (use `?` or a
// documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

mod agents;
mod astar;
#[cfg(test)]
mod astar_tests;
mod avoidance;
mod bake;
mod bounds;
#[cfg(test)]
mod bounds_tests;
mod grid;
mod links;
mod obstacle;
mod offmesh;
mod path;
mod settings;
mod snap;
#[cfg(test)]
pub(crate) mod test_support;

pub use agents::complete_off_mesh_link;
pub use agents::state::{
    is_at_target, remaining_corners, remaining_distance, reset_path, WARP_SNAP_DISTANCE,
};
pub use bake::{CellRect, Rebake};
pub use bounds::{NavBounds, BOUNDS_MARGIN, EMPTY_SCENE_HALF_EXTENT};
pub use grid::{
    NavSpan, NavigationGraph, SpanRef, DEFAULT_GRID_SPACING, DEFAULT_MAX_SLOPE, DEFAULT_MAX_STEP,
};
pub use obstacle::{tick_obstacles, ObstacleVolume};
pub use offmesh::{LinkEnd, OffMeshLink, OffMeshLinkData, OffMeshLinkKind};
pub use path::{path_length, NavPath, NavPathStatus, NavRaycastHit};
pub use settings::{
    NavMeshSettings, DEFAULT_AGENT_HEIGHT, DEFAULT_AGENT_RADIUS, DEFAULT_LINK_SPACING,
};
pub use snap::SNAP_RINGS;
