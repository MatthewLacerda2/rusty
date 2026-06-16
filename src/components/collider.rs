//! src/components/collider.rs — Collider component
//!
//! box/sphere/cylinder + world AABB. Unity: Collider. Moved verbatim from the
//! legacy `core/scene.rs`.

use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ColliderShape {
    Box {
        size: Vec3,
    },
    Sphere {
        radius: f32,
    },
    Cylinder {
        radius: f32,
        height: f32,
    },
    /// A collider baked from an imported mesh's rest pose (#77). `convex` selects a
    /// convex hull (cheaper, valid for dynamic bodies) over an exact triangle mesh
    /// (static geometry). `local_min`/`local_max` are the mesh's local AABB, cached
    /// so the world AABB can be computed without the triangle data — the rapier
    /// shape itself is rebuilt from the entity's live mesh vertices at build time,
    /// keeping geometry out of the scene document.
    Mesh {
        convex: bool,
        local_min: Vec3,
        local_max: Vec3,
    },
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
    /// Build a mesh collider baked from an imported mesh's rest-pose bounds (#77).
    /// `convex` chooses a convex hull over a triangle mesh. The world AABB is left
    /// zeroed; the scene recomputes it via [`Self::calculate_world_aabb`] on load.
    pub fn from_mesh_bounds(convex: bool, local_min: Vec3, local_max: Vec3) -> Self {
        Self {
            active: true,
            shape: ColliderShape::Mesh {
                convex,
                local_min,
                local_max,
            },
            is_trigger: false,
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        }
    }

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
            ColliderShape::Mesh {
                local_min,
                local_max,
                ..
            } => {
                let (n, x) = (*local_min, *local_max);
                vec![
                    Vec3::new(n.x, n.y, n.z),
                    Vec3::new(n.x, n.y, x.z),
                    Vec3::new(n.x, x.y, n.z),
                    Vec3::new(n.x, x.y, x.z),
                    Vec3::new(x.x, n.y, n.z),
                    Vec3::new(x.x, n.y, x.z),
                    Vec3::new(x.x, x.y, n.z),
                    Vec3::new(x.x, x.y, x.z),
                ]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_collider_world_aabb_follows_transform() {
        let c = ColliderComponent::from_mesh_bounds(false, Vec3::splat(-1.0), Vec3::splat(1.0));
        assert!(matches!(c.shape, ColliderShape::Mesh { convex: false, .. }));

        // Translate +Y by 5 and scale x2: the local [-1,1] box maps to [-2,2]±offset.
        let world = Mat4::from_scale_rotation_translation(
            Vec3::splat(2.0),
            glam::Quat::IDENTITY,
            Vec3::new(0.0, 5.0, 0.0),
        );
        let (min, max) = c.calculate_world_aabb(world);
        assert!((min - Vec3::new(-2.0, 3.0, -2.0)).length() < 1e-4);
        assert!((max - Vec3::new(2.0, 7.0, 2.0)).length() < 1e-4);
    }
}
