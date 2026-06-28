//! src/navigation/settings.rs — `NavMeshSettings`: per-scene navmesh bake tunables.
//!
//! These are the authorable knobs Unity exposes as per-scene navmesh bake settings.
//! They live in the `navigation` module (a plain data struct with no `scene` deps) so
//! `scene` can serialize them one-way — `scene` already references `navigation`
//! (`scene::lighting::placement`), and this keeps that direction, never the reverse.
//!
//! The bake reads these off the active `Scene` (`Scene::nav_settings`) instead of the
//! historical hardcoded constants, so they take effect on every re-bake. The serde
//! defaults equal those constants, so a pre-#276 scene that omits the block bakes
//! exactly as it did before.

use serde::{Deserialize, Serialize};

use super::grid::{DEFAULT_GRID_SPACING, DEFAULT_MAX_SLOPE, DEFAULT_MAX_STEP};

/// A sensible default agent radius (world units). Consumed at bake time (#277): the bake
/// erodes the walkable surface inward by this radius (clearance off walls/world-edge).
pub const DEFAULT_AGENT_RADIUS: f32 = 0.5;

/// Default agent height (world units) — Unity's default humanoid agent height. Consumed
/// at bake time (#278): a walkable cell whose vertical clearance to the lowest static
/// geometry overhead is less than this height is marked non-walkable (no crawling under
/// a low overhang). With nothing overhead, headroom is ∞, so the pass is a no-op.
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
    /// Agent height (world units). Consumed by the bake (#278): a walkable cell whose
    /// vertical clearance to the lowest static geometry overhead is below this height is
    /// carved out (so an agent can't path under a low overhang / through a crawlspace).
    /// A cell with nothing overhead has infinite headroom, so it is never carved — simple
    /// scenes (floor + walls, no ceilings) bake unchanged.
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The struct `Default` and every field's serde default must equal the historical
    /// bake constants, so an older scene missing the block bakes exactly as before.
    #[test]
    fn defaults_equal_bake_constants() {
        let d = NavMeshSettings::default();
        assert_eq!(d.max_step, DEFAULT_MAX_STEP);
        assert_eq!(d.max_slope, DEFAULT_MAX_SLOPE);
        assert_eq!(d.grid_spacing, DEFAULT_GRID_SPACING);
        assert_eq!(d.agent_radius, DEFAULT_AGENT_RADIUS);
        assert_eq!(d.agent_height, DEFAULT_AGENT_HEIGHT);
    }

    /// A JSON object missing every field deserializes to the defaults (back-compat).
    #[test]
    fn empty_json_object_fills_defaults() {
        let s: NavMeshSettings = serde_json::from_str("{}").expect("empty object loads");
        assert_eq!(s, NavMeshSettings::default());
    }

    /// A pre-#278 scene that carries the other knobs but omits `agent_height` still loads,
    /// filling the new field with its serde default (back-compat for the height addition).
    #[test]
    fn legacy_json_without_agent_height_uses_default() {
        let json = r#"{"agent_radius":0.5,"max_slope":1.0,"max_step":0.5,"grid_spacing":1.0}"#;
        let s: NavMeshSettings = serde_json::from_str(json).expect("legacy object loads");
        assert_eq!(s.agent_height, DEFAULT_AGENT_HEIGHT);
        assert_eq!(s, NavMeshSettings::default());
    }

    #[test]
    fn round_trips_through_json() {
        let s = NavMeshSettings {
            agent_radius: 0.7,
            agent_height: 1.8,
            max_slope: 0.5,
            max_step: 0.25,
            grid_spacing: 2.0,
        };
        let json = serde_json::to_string(&s).expect("serialize");
        let back: NavMeshSettings = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(s, back);
    }
}
