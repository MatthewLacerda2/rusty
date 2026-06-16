//! src/scene/collision_matrix.rs — the project-wide layer collision matrix.
//!
//! Unity's Layer Collision Matrix: a symmetric NxN table over the 32 shared Layers
//! (#90) deciding which layer collides with which. It drives rapier's
//! `InteractionGroups` on collider build (#91), so two bodies on layers whose cell
//! is unchecked never generate contacts. Serialized with the project so the choices
//! persist across save/load.
//!
//! Row `i` is a `u32` membership mask: bit `j` set means "layer `i` collides with
//! layer `j`". The matrix is kept symmetric, so `can_collide(i, j) == can_collide(j, i)`.

use serde::{Deserialize, Serialize};

use super::layers::LAYER_COUNT;

/// Symmetric collision table over the [`LAYER_COUNT`] shared layers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollisionMatrix {
    rows: [u32; LAYER_COUNT],
}

impl Default for CollisionMatrix {
    /// Everything collides with everything (Unity's default matrix), so pre-#91
    /// scenes keep their current "all pairs collide" behaviour.
    fn default() -> Self {
        Self {
            rows: [u32::MAX; LAYER_COUNT],
        }
    }
}

impl CollisionMatrix {
    /// Whether layers `a` and `b` are allowed to collide.
    pub fn can_collide(&self, a: u8, b: u8) -> bool {
        let (ai, bi) = (a as usize, b as usize);
        ai < LAYER_COUNT && bi < LAYER_COUNT && self.rows[ai] & (1u32 << b) != 0
    }

    /// Toggle the (symmetric) collision between layers `a` and `b`. Out-of-range
    /// indices are ignored.
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

    /// The membership mask of every layer `layer` collides with — the `filter` half
    /// of the rapier `InteractionGroups` for a collider on `layer`.
    pub fn filter_mask(&self, layer: u8) -> u32 {
        if (layer as usize) < LAYER_COUNT {
            self.rows[layer as usize]
        } else {
            0
        }
    }
}
