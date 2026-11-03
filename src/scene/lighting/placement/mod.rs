//! src/scene/lighting/placement/ — Deterministic auto-placement of IBL probes.
//!
//! The "place well" half of the one-button bake (#246): given the STATIC scene (and,
//! when available, the baked nav surface's reachable floor), compute where light
//! probes and reflection probes should sit. This is a PURE, deterministic function of
//! the scene and the data the bake hands over — grids, a floor-plan watershed and
//! rays through an occupancy grid, no RNG and no wall-clock, scan order throughout —
//! so the same level always yields the same probe layout.
//!
//! The bake itself (rendering each probe → SH / prefiltered cubemap) is the render/dev
//! half and lives in `dev::bake::lighting`; this module only decides POSITIONS + bounds.
//!
//! Both sets are planned from one [`Site`] (#1737): the **floor plan** (`floor.rs`:
//! the nav surface's reachable floor, handed over as data to keep the arrow
//! navigation → scene, #721, else rasterized from the static colliders), with every
//! floor under a liquid dropped (`liquid.rs`), and a 3D **occupancy grid** of the
//! static colliders (`occupancy/`).
//!   * **Reflection probes** — one per room (#1584): the floor plan is cut into rooms
//!     at its doorways (`rooms.rs`), and each room gets probes at eye height, clear
//!     of geometry, with boxes fitted to it; a cluttered room gets more, a room open
//!     to the sky one without box projection (`room_probes.rs`, `enclosure.rs`).
//!   * **Light probes** — a lattice through the air over the floors, denser near
//!     geometry, pushed out of it, never above a ceiling (`light.rs`). With nothing
//!     solid to build a site from, a regular grid over the walkable extent
//!     (`grid.rs`).
//!
//! Probes marked manual are kept by the caller; placement plans around them.
//!
//! This module is pure data + math — deterministic, sim-reachable, no RNG/clock.

mod enclosure;
mod floor;
mod grid;
mod islands;
mod light;
mod liquid;
mod occupancy;
mod reflection;
mod room_probes;
mod rooms;
#[cfg(test)]
pub(crate) mod test_level;

use glam::Vec3;

pub use self::floor::{FloorCell, FloorGrid};
pub use self::grid::{plan_light_probes, static_scene_aabb, ProbeGridPlan};
pub use self::liquid::{liquids, Liquid};
pub use self::occupancy::{Occupancy, Solid};
pub use self::reflection::{plan_reflection_probes, ReflectionPlacement, MIN_REGION};
pub use self::room_probes::EYE_HEIGHT;
pub use self::rooms::RoomParams;
use crate::scene::Scene;

/// A reflection-probe request: capture position (eye height over the room's middle)
/// plus the parallax box corners fitted to the room.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReflectionPlan {
    pub position: Vec3,
    pub box_min: Vec3,
    pub box_max: Vec3,
    /// Unity's *Box Projection*: off for a room open to the sky (#1737).
    pub box_projection: bool,
}

/// Vertical headroom added above the placement bounds so probes also cover the air an
/// actor's head/torso occupies, not only the floor it stands on. World units.
pub const VERTICAL_HEADROOM: f32 = 3.0;

/// What placement reads of a level besides the scene (#1737): where actors stand
/// (its floor plan, liquids drowned) and what is solid around them (its occupancy
/// grid). Built once per bake; both probe sets are planned from it.
pub struct Site {
    pub floor: FloorGrid,
    pub occupancy: Occupancy,
}

impl Site {
    /// The site of `scene`: `floor` is the nav surface's plan of reachable floor
    /// (`NavigationGraph::floor_grid`), else the plan rasterized from the static
    /// colliders at `floor_cell`; `solids` are the static colliders' triangles,
    /// else their world boxes. Every floor under a liquid is dropped
    /// (`liquid.rs`). `None` with nothing solid or nothing to stand on.
    pub fn survey(
        scene: &Scene,
        floor: Option<FloorGrid>,
        solids: Option<&[Solid]>,
        floor_cell: f32,
    ) -> Option<Self> {
        let occupancy = match solids {
            Some(solids) => Occupancy::build(solids, occupancy::CELL),
            None => Occupancy::from_static_boxes(scene, occupancy::CELL),
        }?;
        let mut floor = floor.or_else(|| FloorGrid::from_static(scene, floor_cell))?;
        floor.drown(&liquid::liquids(scene));
        Some(Self { floor, occupancy })
    }

    /// The light probes for this site at `spacing` (half of it near geometry),
    /// planned around the `manual` ones (`light.rs`).
    pub fn plan_light_probes(&self, spacing: f32, manual: &[Vec3]) -> Vec<Vec3> {
        light::plan_light_set(&self.floor, &self.occupancy, spacing, manual)
    }
}

#[cfg(test)]
#[path = "../placement_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "limits_tests.rs"]
mod limits_tests;

#[cfg(test)]
#[path = "enclosure_tests.rs"]
mod enclosure_tests;
#[cfg(test)]
#[path = "light_tests.rs"]
mod light_tests;
#[cfg(test)]
#[path = "liquid_tests.rs"]
mod liquid_tests;
#[cfg(test)]
#[path = "reflection_tests.rs"]
mod reflection_tests;
#[cfg(test)]
pub(crate) mod test_sites;
