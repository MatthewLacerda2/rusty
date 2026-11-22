mod area;
mod bounds;
mod saved;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

pub use area::{
    area_index, clamp_cost, default_areas, NavArea, ALL_AREAS, MAX_AREAS, MIN_AREA_COST,
    NOT_WALKABLE_AREA, WALKABLE_AREA,
};
pub use bounds::{NavBounds, EMPTY_SCENE_HALF_EXTENT};
pub use saved::{
    apply_navmesh, load_navmesh, navmesh_path, read_navmesh, save_navmesh, NavmeshFile,
    SavedNavmesh,
};

// Lengths here are metres.
/// The default `CharacterController::step_offset`, so the default navmesh never links
/// a step the default walker cannot climb (Unity's agent, with no controller, uses 0.4).
pub const DEFAULT_MAX_STEP: f32 = 0.3;
/// Rise per unit of horizontal travel: 1.0 is 45°.
pub const DEFAULT_MAX_SLOPE: f32 = 1.0;
/// Half the default agent radius, inside Recast's radius/2 to radius/3 guidance.
pub const DEFAULT_GRID_SPACING: f32 = 0.25;

pub const DEFAULT_AGENT_RADIUS: f32 = 0.5;

pub const DEFAULT_AGENT_HEIGHT: f32 = 2.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NavMeshSettings {
    #[serde(default = "default_agent_radius")]
    pub agent_radius: f32,
    #[serde(default = "default_agent_height")]
    pub agent_height: f32,
    #[serde(default = "default_max_slope")]
    pub max_slope: f32,
    #[serde(default = "default_max_step")]
    pub max_step: f32,
    #[serde(default = "default_grid_spacing")]
    pub grid_spacing: f32,
    #[serde(default)]
    pub bounds: Option<NavBounds>,
    // The generated links' reach (Unity's Drop Height, Jump Distance and Jump Height);
    // 0 makes no link of that kind.
    #[serde(default)]
    pub drop_height: f32,
    #[serde(default)]
    pub jump_distance: f32,
    #[serde(default)]
    pub jump_height: f32,
    #[serde(default = "default_link_spacing")]
    pub link_spacing: f32,
    #[serde(default = "default_areas")]
    pub areas: Vec<NavArea>,
}

impl NavMeshSettings {
    pub fn bakes_like(&self, other: &Self) -> bool {
        let strip = |s: &Self| Self {
            areas: Vec::new(),
            ..s.clone()
        };
        strip(self) == strip(other)
    }
}

/// Metres between generated links along one edge.
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
