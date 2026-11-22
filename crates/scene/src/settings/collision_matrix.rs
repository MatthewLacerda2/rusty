//! Unity's Layer Collision Matrix: row `i` is the mask of layers `i` collides with, kept
//! symmetric by `set_collision`.

use serde::{Deserialize, Serialize};

use super::layers::LAYER_COUNT;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CollisionMatrix {
    rows: [u32; LAYER_COUNT],
}

impl Default for CollisionMatrix {
    fn default() -> Self {
        Self {
            rows: [u32::MAX; LAYER_COUNT],
        }
    }
}

impl CollisionMatrix {
    pub fn can_collide(&self, a: u8, b: u8) -> bool {
        let (ai, bi) = (a as usize, b as usize);
        ai < LAYER_COUNT && bi < LAYER_COUNT && self.rows[ai] & (1u32 << b) != 0
    }

    pub fn set_collision(&mut self, a: u8, b: u8, enabled: bool) {
        let (ai, bi) = (a as usize, b as usize);
        if ai >= LAYER_COUNT || bi >= LAYER_COUNT {
            return;
        }
        if enabled {
            self.rows[ai] |= 1u32 << b;
            self.rows[bi] |= 1u32 << a;
        } else {
            self.rows[ai] &= !(1u32 << b);
            self.rows[bi] &= !(1u32 << a);
        }
    }

    /// The `filter` half of a collider's rapier `InteractionGroups`.
    pub fn filter_mask(&self, layer: u8) -> u32 {
        if (layer as usize) < LAYER_COUNT {
            self.rows[layer as usize]
        } else {
            0
        }
    }
}
