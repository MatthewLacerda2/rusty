//! src/components/nav_modifier.rs — NavMeshModifierVolume (#460).
//!
//! Unity's `NavMeshModifierVolume`: a box that assigns a navigation **area** to the
//! walkable surface inside it when the navmesh bakes — a strip of mud that costs
//! more to cross, a street an agent should avoid, a zone some agents may not
//! enter. The box is `center` ± `size / 2` in the entity's local space, so it moves,
//! turns and scales with the Transform. Area `NotWalkable` (1) removes the surface
//! instead. The area's cost lives in the scene's area table
//! (`NavMeshSettings::areas`), so a script can change it at runtime without a rebake.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// A modifier volume. See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NavMeshModifierVolumeComponent {
    /// An inactive volume assigns nothing (the surface keeps its own area).
    pub active: bool,
    /// The box's centre, local to the entity.
    pub center: Vec3,
    /// The box's full extent along each local axis.
    pub size: Vec3,
    /// The area id assigned to the spans whose floor lies inside the box.
    pub area: u8,
}

impl Default for NavMeshModifierVolumeComponent {
    /// Unity's defaults: a 4 × 3 × 4 box raised 1 m, assigning `Walkable`.
    fn default() -> Self {
        Self {
            active: true,
            center: Vec3::new(0.0, 1.0, 0.0),
            size: Vec3::new(4.0, 3.0, 4.0),
            area: 0,
        }
    }
}
