//! An Area (Rectangle) light as the bake samples it (#1674): Unity's built-in,
//! baked-only area light.
//!
//! The emitter is a `width × height` rectangle facing `forward` (its local −Z),
//! one-sided, with `radiance` (colour × intensity) per unit area: a patch `dA`
//! seen at distance `d` and angle `θ` off its normal sends `radiance · cos θ · dA /
//! (d² + 1)` toward a receiver, the point light's falloff with the patch's
//! Lambertian cosine. Integrated, a small or far panel acts as a point light of
//! intensity `intensity × area` straight ahead, fading with `cos θ` to its sides,
//! which is the energy rule the tests hold. `range` cuts it off, measured from the
//! centre, as for point and spot lights.
//!
//! The rectangle is integrated by sampling points on it: the lightmap tracer draws
//! jittered, stratified points from the texel's seeded stream, so penumbrae come out
//! soft and the bake stays a pure function of its inputs; the probe bake uses
//! [`GRID`] fixed cell centres.

use glam::{Vec2, Vec3};

/// Cells per side of the fixed grid the probe bake integrates a rectangle over:
/// `GRID²` shadow rays per area light per probe.
pub const GRID: u32 = 8;

/// The rectangle, in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AreaShape {
    pub center: Vec3,
    /// The full width edge (local +X scaled by the width).
    pub right: Vec3,
    /// The full height edge (local +Y scaled by the height).
    pub up: Vec3,
    /// The unit facing (local −Z): only what lies in front is lit.
    pub forward: Vec3,
    pub range: f32,
}

impl AreaShape {
    /// The point at `u` in `[0, 1]²` across the rectangle.
    pub fn point(&self, u: Vec2) -> Vec3 {
        self.center + self.right * (u.x - 0.5) + self.up * (u.y - 0.5)
    }

    /// The emitting area, in square world units.
    pub fn area(&self) -> f32 {
        self.right.length() * self.up.length()
    }

    /// Unit direction from `receiver` to the rectangle's point at `u`, its distance,
    /// and the radiance per unit `radiance` that one sample stands for (the whole
    /// area's, so the mean over samples is the integral). `None` past `range`, or
    /// behind the one-sided emitter.
    pub fn arrival(&self, receiver: Vec3, u: Vec2) -> Option<(Vec3, f32, f32)> {
        if receiver.distance(self.center) > self.range {
            return None;
        }
        let offset = self.point(u) - receiver;
        let d = offset.length();
        if d <= 0.0 {
            return None;
        }
        let dir = offset / d;
        let cos_emit = -dir.dot(self.forward);
        (cos_emit > 0.0).then(|| (dir, d, cos_emit * self.area() / (d * d + 1.0)))
    }
}

/// `n` or more jittered, stratified points in `[0, 1)²`, one per cell of the
/// smallest square grid with at least `n` cells, each drawn with two values from
/// `next` (the texel's seeded stream).
pub fn stratified(n: u32, mut next: impl FnMut() -> f32) -> Vec<Vec2> {
    let m = (n.max(1) as f32).sqrt().ceil() as u32;
    let mut out = Vec::with_capacity((m * m) as usize);
    for j in 0..m {
        for i in 0..m {
            let jitter = Vec2::new(next(), next());
            out.push((Vec2::new(i as f32, j as f32) + jitter) / m as f32);
        }
    }
    out
}

/// The [`GRID`]² cell centres the probe bake samples a rectangle at.
pub fn grid() -> impl Iterator<Item = Vec2> {
    (0..GRID * GRID).map(|k| {
        let cell = Vec2::new((k % GRID) as f32, (k / GRID) as f32);
        (cell + 0.5) / GRID as f32
    })
}
