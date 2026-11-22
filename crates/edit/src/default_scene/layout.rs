//! Lengths in metres.

pub const DECK_TOP: f32 = 0.0;
pub const SLAB_BOTTOM: f32 = -2.0;
pub const YARD_HALF: (f32, f32) = (15.0, 22.5);
pub const WALL_HEIGHT: f32 = 4.0;
pub const WALL_THICK: f32 = 0.5;

pub const POOL_HALF: (f32, f32) = (5.0, 11.0);
pub const POOL_DEPTH: f32 = 1.8;
pub const POOL_FLOOR: f32 = DECK_TOP - POOL_DEPTH;
/// Just under the deck, so the water's edges never z-fight it.
pub const WATER_LEVEL: f32 = DECK_TOP - 0.15;

pub const RAMP_HALF_WIDTH: f32 = 2.0;
/// With `POOL_DEPTH`, a 0.3 grade (about 17°): under the navmesh bake's max slope (1.0 rise per
/// run) and the controller's 45° limit.
pub const RAMP_RUN: f32 = 6.0;
/// A ramp's underside sinks into the deck and the floor, out of sight.
pub const RAMP_THICK: f32 = 0.4;

pub const JACUZZI_AT: (f32, f32) = (10.0, 0.0);
pub const JACUZZI_HALF: f32 = 1.5;
/// Above the controller's 0.3 m step offset, so getting in is a jump.
pub const RIM_HEIGHT: f32 = 0.9;
pub const RIM_THICK: f32 = 0.3;
pub const BASIN_TOP: f32 = 0.3;

/// Far enough from `Wall_South` that the follow camera, 4.5 m behind, starts 2 m clear of its
/// inner face.
pub const PLAYER_SPAWN: [f32; 3] = [0.0, 1.5, -15.5];
pub const ENEMY_SPAWN: [f32; 3] = [0.0, 1.0, 20.0];

pub fn ramp_grade() -> f32 {
    POOL_DEPTH / RAMP_RUN
}

/// Ignores the crates and the jacuzzi, so callers keep to the open deck and the pool.
pub fn ground_at(x: f32, z: f32) -> Option<f32> {
    if x.abs() > YARD_HALF.0 || z.abs() > YARD_HALF.1 {
        return None;
    }
    if x.abs() > POOL_HALF.0 || z.abs() > POOL_HALF.1 {
        return Some(DECK_TOP);
    }
    let into_pool = POOL_HALF.1 - z.abs();
    if x.abs() <= RAMP_HALF_WIDTH && into_pool < RAMP_RUN {
        return Some(DECK_TOP - into_pool * ramp_grade());
    }
    Some(POOL_FLOOR)
}
