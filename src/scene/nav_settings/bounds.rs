//! src/scene/nav_settings/bounds.rs — `NavBounds`, the XZ rectangle a navmesh covers.
//!
//! The authored override (`NavMeshSettings::bounds`, Unity's nav volume) is saved with
//! the scene, so the type is scene data. Resolving the bounds a bake actually uses
//! (override, else the static geometry's extent, #452) is `navigation::bounds`.

use serde::{Deserialize, Serialize};

/// Half-extent of the default box an empty scene (no static geometry) bakes over — the
/// historical hardcoded ±20 bounds.
pub const EMPTY_SCENE_HALF_EXTENT: f32 = 20.0;

/// An XZ rectangle in world units: the area the navmesh grid covers.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NavBounds {
    pub min_x: f32,
    pub max_x: f32,
    pub min_z: f32,
    pub max_z: f32,
}

impl NavBounds {
    pub const fn new(min_x: f32, max_x: f32, min_z: f32, max_z: f32) -> Self {
        Self {
            min_x,
            max_x,
            min_z,
            max_z,
        }
    }

    /// The default box an empty scene bakes over (±[`EMPTY_SCENE_HALF_EXTENT`]).
    pub const EMPTY_SCENE: Self = Self {
        min_x: -EMPTY_SCENE_HALF_EXTENT,
        max_x: EMPTY_SCENE_HALF_EXTENT,
        min_z: -EMPTY_SCENE_HALF_EXTENT,
        max_z: EMPTY_SCENE_HALF_EXTENT,
    };

    /// Finite, with a positive extent on both axes. The bake ignores an invalid
    /// override (falls back to the derived bounds) so it stays a total function.
    pub fn is_valid(&self) -> bool {
        let finite = [self.min_x, self.max_x, self.min_z, self.max_z]
            .iter()
            .all(|v| v.is_finite());
        finite && self.min_x < self.max_x && self.min_z < self.max_z
    }
}
