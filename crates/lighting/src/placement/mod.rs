mod enclosure;
mod grid;
mod islands;
mod light;
mod liquid;
mod occupancy;
mod reflection;
mod room_probes;
mod rooms;
#[doc(hidden)]
pub mod test_level;

use glam::Vec3;

pub use self::grid::{plan_light_probes, static_scene_aabb, ProbeGridPlan};
pub use self::liquid::{liquids, Liquid};
pub use self::occupancy::{Occupancy, Solid};
pub use self::reflection::{plan_reflection_probes, ReflectionPlacement, MIN_REGION};
pub use self::room_probes::EYE_HEIGHT;
pub use self::rooms::RoomParams;
use rusty_scene::lighting::floor::FloorGrid;
use rusty_scene::Scene;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReflectionPlan {
    pub position: Vec3,
    pub box_min: Vec3,
    pub box_max: Vec3,
    pub box_projection: bool,
    pub importance: i32,
}

pub const OPEN_AIR_IMPORTANCE: i32 = 0;

pub const VERTICAL_HEADROOM: f32 = 3.0;

pub struct Site {
    pub floor: FloorGrid,
    pub occupancy: Occupancy,
}

impl Site {
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
        liquid::drown(&mut floor, &liquid::liquids(scene));
        Some(Self { floor, occupancy })
    }

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
#[path = "budget_tests.rs"]
mod budget_tests;
#[cfg(test)]
#[path = "enclosure_tests.rs"]
mod enclosure_tests;
#[cfg(test)]
#[path = "light_tests.rs"]
mod light_tests;
#[cfg(test)]
#[path = "liquid_tests.rs"]
mod liquid_tests;
#[doc(hidden)]
pub mod test_sites;
