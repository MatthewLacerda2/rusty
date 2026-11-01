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

use crate::components::LightComponent;
use crate::scene::Scene;

/// Cells per side of the fixed grid the probe bake integrates a rectangle over:
/// `GRID²` shadow rays per area light per probe.
pub const GRID: u32 = 8;

/// How far in front of the rectangle (along `forward`) its sample points sit, so
/// the shadow ray toward one stops short of a surface the panel is mounted on.
pub const EMIT_OFFSET: f32 = 1e-3;

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
    /// The rectangle of Area light `light` on entity `id`, in world space: its
    /// `area_size` along the entity's local X and Y, facing its local −Z (−Y when
    /// the transform collapses that). The one constructor the bake and the
    /// editor's gizmo share, so the two never disagree on where a panel is.
    pub fn of(scene: &Scene, id: u32, light: &LightComponent) -> Self {
        let axis = |local| scene.world.transform_direction(id, local);
        Self {
            center: scene.world.world_position(id),
            right: axis(Vec3::X).normalize_or_zero() * light.area_size.x,
            up: axis(Vec3::Y).normalize_or_zero() * light.area_size.y,
            forward: axis(Vec3::NEG_Z).try_normalize().unwrap_or(Vec3::NEG_Y),
            range: light.range,
        }
    }

    /// The point at `u` in `[0, 1]²` across the rectangle.
    pub fn point(&self, u: Vec2) -> Vec3 {
        self.center + self.right * (u.x - 0.5) + self.up * (u.y - 0.5)
    }

    /// Whether any of the rectangle can light `receiver`: within `range` of the
    /// centre and in front of the one-sided emitter. Checked before any sample
    /// point is drawn, so a receiver out of reach costs no draws.
    pub fn reaches(&self, receiver: Vec3) -> bool {
        receiver.distance(self.center) <= self.range
            && (receiver - self.center).dot(self.forward) > 0.0
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
        let offset = self.point(u) + self.forward * EMIT_OFFSET - receiver;
        let d = offset.length();
        if d <= 0.0 {
            return None;
        }
        let dir = offset / d;
        let cos_emit = -dir.dot(self.forward);
        (cos_emit > 0.0).then(|| (dir, d, cos_emit * self.area() / (d * d + 1.0)))
    }
}

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
