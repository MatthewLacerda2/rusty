//! src/components/collider.rs — Collider component
//!
//! box/sphere/cylinder + world AABB. Unity: Collider. Moved verbatim from the
//! legacy `core/scene.rs`.

use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ColliderShape {
    Box { size: Vec3 },
    Sphere { radius: f32 },
    Cylinder { radius: f32, height: f32 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColliderComponent {
    pub active: bool,
    pub shape: ColliderShape,
    pub is_trigger: bool,
    // Cached world space bounds
    pub aabb_min: Vec3,
    pub aabb_max: Vec3,
}

impl ColliderComponent {
    pub fn calculate_world_aabb(&self, world_mat: Mat4) -> (Vec3, Vec3) {
        let local_corners = match &self.shape {
            ColliderShape::Box { size } => {
                let h = *size * 0.5;
                vec![
                    Vec3::new(-h.x, -h.y, -h.z),
                    Vec3::new(-h.x, -h.y, h.z),
                    Vec3::new(-h.x, h.y, -h.z),
                    Vec3::new(-h.x, h.y, h.z),
                    Vec3::new(h.x, -h.y, -h.z),
                    Vec3::new(h.x, -h.y, h.z),
                    Vec3::new(h.x, h.y, -h.z),
                    Vec3::new(h.x, h.y, h.z),
                ]
            }
            ColliderShape::Sphere { radius } => {
                let r = *radius;
                vec![Vec3::new(-r, -r, -r), Vec3::new(r, r, r)]
            }
            ColliderShape::Cylinder { radius, height } => {
                let r = *radius;
                let h = *height * 0.5;
                vec![Vec3::new(-r, -h, -r), Vec3::new(r, h, r)]
            }
        };

        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);

        for &corner in &local_corners {
            let world_pos = world_mat.transform_point3(corner);
            min = min.min(world_pos);
            max = max.max(world_pos);
        }

        (min, max)
    }
}
