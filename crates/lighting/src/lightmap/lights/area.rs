//! An Area (Rectangle) light as the bake samples it (#1674): Unity's built-in,
//! baked-only area light.
//!
//! The emitter is a `width × height` rectangle facing `forward` (its local −Z),
//! one-sided, with `radiance` (colour × intensity) per unit area: a patch `dA`
//! seen at distance `d` and angle `θ` off its normal sends `radiance · cos θ · dA ·
//! window(d_c, range) / d²` toward a receiver `d_c` from the panel's centre: the
//! point light's inverse square on the patch's own distance, with the patch's
//! Lambertian cosine, and the windowed falloff's window ([`falloff_window`](rusty_components::falloff_window), #1699)
//! taken once, from the centre, so every patch fades out together and the light
//! on a floor is continuous where `range` ends it. Integrated, a small or far
//! panel acts as a point light of intensity `intensity × area` straight ahead,
//! fading with `cos θ` to its sides, which is the energy rule the tests hold.
//! `range` is where it reaches zero: no receiver that far from the centre is lit.
//!
//! The rectangle is integrated by sampling points on it: the lightmap tracer draws
//! jittered, stratified points from the texel's seeded stream, so penumbrae come out
//! soft and the bake stays a pure function of its inputs; the probe bake uses
//! [`GRID`] fixed cell centres.

use glam::Vec2;

/// Cells per side of the fixed grid the probe bake integrates a rectangle over:
/// `GRID²` shadow rays per area light per probe.
pub const GRID: u32 = 8;

pub use rusty_scene::lighting::area::{AreaShape, EMIT_OFFSET};

/// Blocks per side of the probe a lightmap texel casts at an Area light before
/// the rest of its stratified set (#2355, `trace::own`): the full set's `m × m`
/// cells are split into `PROBE × PROBE` blocks ([`block_span`]), and the probe
/// casts the ray of one cell picked in each, plus one per corner. Its stratified
/// rays are points of the full set, so where the set follows they count toward
/// it. A set too small for the probe to pay (`trace::own`'s `probes`) skips it.
pub const PROBE: u32 = 4;

/// The side of the smallest square grid with at least `n` cells: [`cell_point`]
/// places a point in each of its `m²` cells.
pub fn strata(n: u32) -> u32 {
    (n.max(1) as f32).sqrt().ceil() as u32
}

/// Cell `k` (row order) of an `m × m` grid over `[0, 1)²`, at `jitter` (each in
/// `[0, 1)`) inside it: one jittered, stratified point, its two values drawn from
/// the texel's seeded stream.
pub fn cell_point(k: u32, m: u32, jitter: Vec2) -> Vec2 {
    (Vec2::new((k % m) as f32, (k / m) as f32) + jitter) / m as f32
}

/// The cells `[lo, hi)` of an `m`-cell side that probe block `b` spans: cell `i`
/// is in block `i * PROBE / m`, so for `m ≥ PROBE` every block holds at least one.
pub fn block_span(b: u32, m: u32) -> (u32, u32) {
    ((b * m).div_ceil(PROBE), ((b + 1) * m).div_ceil(PROBE))
}

/// The probe block (row order, of `PROBE²`) cell `k` of an `m × m` grid is in.
pub fn block_of(k: u32, m: u32) -> usize {
    ((k / m * PROBE / m) * PROBE + k % m * PROBE / m) as usize
}

/// The [`GRID`]² cell centres the probe bake samples a rectangle at.
pub fn grid() -> impl Iterator<Item = Vec2> {
    (0..GRID * GRID).map(|k| {
        let cell = Vec2::new((k % GRID) as f32, (k / GRID) as f32);
        (cell + 0.5) / GRID as f32
    })
}
