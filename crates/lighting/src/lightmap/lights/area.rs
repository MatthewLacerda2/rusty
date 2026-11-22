//! Unity's baked-only area light: a one-sided `width × height` rectangle facing `forward` (local
//! −Z) with `radiance` per unit area. A patch `dA` at distance `d`, angle `θ` off the normal sends
//! `radiance · cos θ · dA · window(d_c, range) / d²`; the window
//! ([`falloff_window`](rusty_components::falloff_window)) is taken once at the centre distance
//! `d_c`, so every patch fades together and a floor's light is continuous where `range` ends it.

use glam::Vec2;

/// Cells per side: `GRID²` shadow rays per probe.
pub const GRID: u32 = 8;

pub use rusty_scene::lighting::area::{AreaShape, EMIT_OFFSET};

/// The full set's `m × m` cells split into `PROBE × PROBE` blocks ([`block_span`]); the probe casts
/// one cell per block plus one per corner, counted toward the set.
pub const PROBE: u32 = 4;

pub fn strata(n: u32) -> u32 {
    (n.max(1) as f32).sqrt().ceil() as u32
}

pub fn cell_point(k: u32, m: u32, jitter: Vec2) -> Vec2 {
    (Vec2::new((k % m) as f32, (k / m) as f32) + jitter) / m as f32
}

pub fn block_span(b: u32, m: u32) -> (u32, u32) {
    ((b * m).div_ceil(PROBE), ((b + 1) * m).div_ceil(PROBE))
}

pub fn block_of(k: u32, m: u32) -> usize {
    ((k / m * PROBE / m) * PROBE + k % m * PROBE / m) as usize
}

pub fn grid() -> impl Iterator<Item = Vec2> {
    (0..GRID * GRID).map(|k| {
        let cell = Vec2::new((k % GRID) as f32, (k / GRID) as f32);
        (cell + 0.5) / GRID as f32
    })
}
