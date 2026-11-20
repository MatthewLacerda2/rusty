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

/// Cells per side of the probe a lightmap texel casts at an Area light before
/// its full stratified set (#2355, `trace::direct`): `PROBE²` stratified shadow
/// rays and one per corner decide whether the panel is fully seen, fully hidden,
/// or in penumbra. A direct sample count of `PROBE²` or fewer skips the probe.
pub const PROBE: u32 = 4;

/// The side of the smallest square grid with at least `n` cells: [`stratified`]
/// yields its square.
pub fn strata(n: u32) -> u32 {
    (n.max(1) as f32).sqrt().ceil() as u32
}

/// `m²` jittered, stratified points in `[0, 1)²`, one per cell of an `m × m`
/// grid in row order, each drawn lazily with two values from `next` (the texel's
/// seeded stream). Allocates nothing.
pub fn stratified(m: u32, mut next: impl FnMut() -> f32) -> impl Iterator<Item = Vec2> {
    (0..m * m).map(move |k| {
        let jitter = Vec2::new(next(), next());
        (Vec2::new((k % m) as f32, (k / m) as f32) + jitter) / m as f32
    })
}

/// The [`GRID`]² cell centres the probe bake samples a rectangle at.
pub fn grid() -> impl Iterator<Item = Vec2> {
    (0..GRID * GRID).map(|k| {
        let cell = Vec2::new((k % GRID) as f32, (k / GRID) as f32);
        (cell + 0.5) / GRID as f32
    })
}
