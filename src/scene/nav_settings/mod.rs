//! src/scene/nav_settings/ — `NavMeshSettings`: per-scene navmesh bake tunables.
//!
//! These are the authorable knobs Unity exposes as per-scene navmesh bake settings.
//! They are plain serde data the scene saves, so they live with the scene, beside
//! `FogSettings`: the arrow is navigation → scene, never the reverse (#721). The bake
//! (`navigation`) reads them off the active `Scene` (`Scene::nav_settings`) and takes
//! its defaults from the constants here, so they take effect on every re-bake. The
//! serde defaults equal those constants, so a pre-#276 scene that omits the block
//! bakes exactly as it did before.
//!
//! Submodules: `area` (the area table's rows and limits, #460) and `bounds` (the
//! authored XZ nav volume, #452). `navigation` re-exports all of it under its old
//! paths.

mod area;
mod bounds;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

pub use area::{
    default_areas, NavArea, ALL_AREAS, MAX_AREAS, MIN_AREA_COST, NOT_WALKABLE_AREA, WALKABLE_AREA,
};
pub use bounds::{NavBounds, EMPTY_SCENE_HALF_EXTENT};

/// Default maximum height an agent can step up/down between two adjacent cells
/// while still treating them as connected (stairs, curbs). World units.
pub const DEFAULT_MAX_STEP: f32 = 0.5;
/// Default maximum walkable slope, expressed as a height delta per cell of
/// horizontal travel (i.e. `rise / grid_spacing`). A ramp steeper than this is
/// not traversable. `1.0` ≈ 45° at unit spacing.
pub const DEFAULT_MAX_SLOPE: f32 = 1.0;
/// Default grid cell size in world units — the spacing every runtime
/// `NavigationGraph` is created with. The [`NavMeshSettings`] default matches this so
/// an unconfigured scene bakes at the historical resolution.
pub const DEFAULT_GRID_SPACING: f32 = 1.0;

/// A sensible default agent radius (world units). Consumed at bake time (#277): the bake
/// erodes the walkable surface inward by this radius (clearance off walls/world-edge).
pub const DEFAULT_AGENT_RADIUS: f32 = 0.5;

/// Default agent height (world units) — Unity's default humanoid agent height. Consumed
/// at bake time (#278): a span with less open space above it than this height is not
/// walkable (no crawling under a low overhang).
pub const DEFAULT_AGENT_HEIGHT: f32 = 2.0;

/// Per-scene navmesh bake settings (Unity's per-scene navmesh bake parameters).
///
/// Serialized on the `Scene` document and editable from the scene inspector + the
/// `Navigation` script API. The `Default` here is the engine's source of truth for
/// the bake tunables: every field's default equals the historical constant, so an
/// older scene missing the block (back-compat) bakes byte-identically to before.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NavMeshSettings {
    /// Agent radius (world units). Consumed by the bake (#277): the walkable surface is
    /// eroded inward by this radius (the standard Recast/Unity meaning) — passages
    /// narrower than ~`2 * radius` close, and the surface keeps clearance off walls and
    /// the world edge. `0` is an exact no-op (the surface hugs geometry as before).
    #[serde(default = "default_agent_radius")]
    pub agent_radius: f32,
    /// Agent height (world units). Consumed by the bake (#278): a span whose open space
    /// up to the next solid above is below this height is dropped (so an agent can't path
    /// under a low overhang / through a crawlspace), and a move between two spans needs it
    /// in their shared gap. A span with nothing above has infinite headroom.
    #[serde(default = "default_agent_height")]
    pub agent_height: f32,
    /// Max walkable grade (rise per unit of horizontal travel). A ramp steeper than
    /// this is not traversable; `1.0` ≈ 45° at unit spacing. Sourced into
    /// `NavigationGraph::max_slope` at bake time.
    #[serde(default = "default_max_slope")]
    pub max_slope: f32,
    /// Max absolute step height between adjacent cells for a move to be allowed
    /// (stairs, curbs). Sourced into `NavigationGraph::max_step` at bake time.
    #[serde(default = "default_max_step")]
    pub max_step: f32,
    /// Grid cell size (world units). Smaller spacing = finer grid = more cells. The
    /// bake rebuilds the grid dimensions from this against the existing bounds when it
    /// differs from the graph's current spacing (see `NavigationGraph::bake`).
    #[serde(default = "default_grid_spacing")]
    pub grid_spacing: f32,
    /// Authored XZ bounds the grid covers (#452, Unity's nav volume). `None` (the
    /// default) derives them at bake time from the static geometry's extent plus a
    /// margin; an invalid override is ignored the same way (see `navigation::bounds`).
    #[serde(default)]
    pub bounds: Option<NavBounds>,
    /// Auto-generated off-mesh links (#462, Unity's "Drop Height"): the furthest an
    /// agent drops off a ledge onto the floor below. `0` (the default) makes no drop
    /// links. See `navigation::offmesh`.
    #[serde(default)]
    pub drop_height: f32,
    /// The widest gap a generated jump link crosses (Unity's "Jump Distance");
    /// `0` (the default) makes no jump links across gaps.
    #[serde(default)]
    pub jump_distance: f32,
    /// The highest ledge a generated jump link climbs onto (a box, a window sill);
    /// `0` (the default) makes no jump links upward.
    #[serde(default)]
    pub jump_height: f32,
    /// Generated links along one edge are thinned to one per this many world units.
    #[serde(default = "default_link_spacing")]
    pub link_spacing: f32,
    /// The area table (#460): area `i` is row `i`, with its name and cost multiplier.
    /// Up to `MAX_AREAS` rows; rows 0 and 1 are the built-in `Walkable` and
    /// `NotWalkable`. Costs are read at search time, so the table is not a bake input.
    #[serde(default = "default_areas")]
    pub areas: Vec<NavArea>,
}

impl NavMeshSettings {
    /// Whether a bake with `other` gives the same spans and links as one with these:
    /// everything but the area table (costs are read at search time) is equal.
    pub fn bakes_like(&self, other: &Self) -> bool {
        let strip = |s: &Self| Self {
            areas: Vec::new(),
            ..s.clone()
        };
        strip(self) == strip(other)
    }
}

/// Default spacing between generated links along one edge (world units).
pub const DEFAULT_LINK_SPACING: f32 = 2.0;

fn default_link_spacing() -> f32 {
    DEFAULT_LINK_SPACING
}

fn default_agent_radius() -> f32 {
    DEFAULT_AGENT_RADIUS
}
fn default_agent_height() -> f32 {
    DEFAULT_AGENT_HEIGHT
}
fn default_max_slope() -> f32 {
    DEFAULT_MAX_SLOPE
}
fn default_max_step() -> f32 {
    DEFAULT_MAX_STEP
}
fn default_grid_spacing() -> f32 {
    DEFAULT_GRID_SPACING
}

impl Default for NavMeshSettings {
    fn default() -> Self {
        Self {
            agent_radius: DEFAULT_AGENT_RADIUS,
            agent_height: DEFAULT_AGENT_HEIGHT,
            max_slope: DEFAULT_MAX_SLOPE,
            max_step: DEFAULT_MAX_STEP,
            grid_spacing: DEFAULT_GRID_SPACING,
            bounds: None,
            drop_height: 0.0,
            jump_distance: 0.0,
            jump_height: 0.0,
            link_spacing: DEFAULT_LINK_SPACING,
            areas: default_areas(),
        }
    }
}
