use serde::{Deserialize, Serialize};

pub const EMPTY_SCENE_HALF_EXTENT: f32 = 20.0;

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

    pub const EMPTY_SCENE: Self = Self {
        min_x: -EMPTY_SCENE_HALF_EXTENT,
        max_x: EMPTY_SCENE_HALF_EXTENT,
        min_z: -EMPTY_SCENE_HALF_EXTENT,
        max_z: EMPTY_SCENE_HALF_EXTENT,
    };

    pub fn is_valid(&self) -> bool {
        let finite = [self.min_x, self.max_x, self.min_z, self.max_z]
            .iter()
            .all(|v| v.is_finite());
        finite && self.min_x < self.max_x && self.min_z < self.max_z
    }
}
