//! src/scene/default_scene/layout.rs — the fy_pool_day yard's measurements (#747).
//!
//! One place for every number the builders share with the tests that walk the
//! yard: change a size here and the geometry and its tests move together.
//!
//! The yard runs along **z**: the Player spawns at the −z end, Enemy_1 at +z, and
//! the pool lies between them with a ramp at each short end on the spawn-to-spawn
//! axis (x = 0), so the straight walk from one spawn to the other goes down one
//! ramp, across the bottom and up the other.

/// The deck's top face: the yard's ground level.
pub const DECK_TOP: f32 = 0.0;
/// The deck slabs' underside, level with the pool floor's.
pub const SLAB_BOTTOM: f32 = -2.0;
/// Half the yard's footprint (x, z), deck edge to deck edge.
pub const YARD_HALF: (f32, f32) = (15.0, 22.5);
/// The perimeter walls stand on the deck's outer edge, inside `YARD_HALF`.
pub const WALL_HEIGHT: f32 = 4.0;
pub const WALL_THICK: f32 = 0.5;

/// Half the pool's footprint (x, z): a 10 × 22 m hole in the deck.
pub const POOL_HALF: (f32, f32) = (5.0, 11.0);
/// How far the pool floor sits below the deck.
pub const POOL_DEPTH: f32 = 1.8;
/// The pool floor's top face.
pub const POOL_FLOOR: f32 = DECK_TOP - POOL_DEPTH;
/// The visual water's surface, just under the deck so the edges don't fight it.
pub const WATER_LEVEL: f32 = DECK_TOP - 0.15;

/// Half a ramp's width (x).
pub const RAMP_HALF_WIDTH: f32 = 2.0;
/// A ramp's horizontal run (z), from the pool's end wall to the flat bottom. 1.8 m
/// over 6 m is a 0.3 grade (≈ 17°): under the bake's max slope (1.0) and the
/// character controller's 45° limit.
pub const RAMP_RUN: f32 = 6.0;
/// A ramp box's thickness; its underside hides inside the deck and the floor.
pub const RAMP_THICK: f32 = 0.4;

/// The jacuzzi's centre on the deck, beside the pool on the +x side.
pub const JACUZZI_AT: (f32, f32) = (10.0, 0.0);
pub const JACUZZI_HALF: f32 = 1.5;
/// The rim's top above the deck: higher than any step, so getting in is a jump.
pub const RIM_HEIGHT: f32 = 0.9;
pub const RIM_THICK: f32 = 0.3;
/// The basin's floor, a shallow tub inside the rim.
pub const BASIN_TOP: f32 = 0.3;

/// Where the Player starts, at the −z spawn, dropped onto the deck. Far enough in
/// from `Wall_South` that the follow camera, 4.5 m behind, starts 2 m clear of the
/// wall's inner face (#852).
pub const PLAYER_SPAWN: [f32; 3] = [0.0, 1.5, -15.5];
/// Where Enemy_1 starts, at the +z spawn; its origin is 1 m above its feet.
pub const ENEMY_SPAWN: [f32; 3] = [0.0, 1.0, 20.0];

/// The ramp's grade: rise over run.
pub fn ramp_grade() -> f32 {
    POOL_DEPTH / RAMP_RUN
}

/// The walkable surface's height at `(x, z)`, by the layout: the deck, a ramp, or the
/// pool floor (`None` outside the yard). Ignores crates and the jacuzzi, so callers
/// keep to the open deck and the pool.
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
