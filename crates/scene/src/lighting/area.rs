use glam::{Vec2, Vec3};

use crate::Scene;
use rusty_components::{falloff_window, LightComponent, FALLOFF_EPSILON_SQ};

pub const EMIT_OFFSET: f32 = 1e-3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AreaShape {
    pub center: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
    pub range: f32,
}

impl AreaShape {
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

    pub fn point(&self, u: Vec2) -> Vec3 {
        self.center + self.right * (u.x - 0.5) + self.up * (u.y - 0.5)
    }

    pub fn reaches(&self, receiver: Vec3) -> bool {
        receiver.distance(self.center) < self.range
            && (receiver - self.center).dot(self.forward) > 0.0
    }

    pub fn area(&self) -> f32 {
        self.right.length() * self.up.length()
    }

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
