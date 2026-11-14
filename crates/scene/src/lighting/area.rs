//! crates/scene/src/lighting/area.rs — an Area (Rectangle) light's rectangle (#1674),
//! in world space, and the light one point of it sends a receiver: what the
//! lightmap bake samples (`rusty-lighting`'s `lightmap::lights::area`, its doc has
//! the falloff), and what the editor's gizmo and the renderer's overlay draw, so
//! none of them disagrees on where a panel is.

use glam::{Vec2, Vec3};

use crate::Scene;
use rusty_components::{falloff_window, LightComponent, FALLOFF_EPSILON_SQ};

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

    /// Whether any of the rectangle can light `receiver`: nearer than `range` to
    /// the centre and in front of the one-sided emitter. Checked before any sample
    /// point is drawn, so a receiver out of reach costs no draws.
    pub fn reaches(&self, receiver: Vec3) -> bool {
        receiver.distance(self.center) < self.range
            && (receiver - self.center).dot(self.forward) > 0.0
    }

    /// The emitting area, in square world units.
    pub fn area(&self) -> f32 {
        self.right.length() * self.up.length()
    }

    /// Unit direction from `receiver` to the rectangle's point at `u`, its distance,
    /// and the radiance per unit `radiance` that one sample stands for (the whole
    /// area's, so the mean over samples is the integral). `None` where it delivers
    /// nothing (at or past `range` from the centre, or behind the one-sided
    /// emitter), so no shadow ray is cast for it.
    pub fn arrival(&self, receiver: Vec3, u: Vec2) -> Option<(Vec3, f32, f32)> {
        let window = falloff_window(receiver.distance(self.center), self.range);
        if window <= 0.0 {
            return None;
        }
        let offset = self.point(u) + self.forward * EMIT_OFFSET - receiver;
        let d = offset.length();
        if d <= 0.0 {
            return None;
        }
        let dir = offset / d;
        let cos_emit = -dir.dot(self.forward);
        let scale = cos_emit * self.area() * window / (d * d).max(FALLOFF_EPSILON_SQ);
        (scale > 0.0).then_some((dir, d, scale))
    }
}
