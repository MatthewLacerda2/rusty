//! src/physics/mod.rs — physics front door.
//!
//! The body/collision simulation is now rapier3d (see `world::PhysicsWorld`),
//! replacing the hand-rolled gravity + box-clipping solver. This module keeps the
//! engine-facing `Ray` type and the scene-level AABB raycast used by the Lua
//! `Physics.Raycast`/`Shoot` bindings (those hold only a `Scene`, not the live
//! `PhysicsWorld`); the in-engine hitscan in `app/play.rs` goes through rapier's
//! query pipeline instead.

mod build;
mod character;
mod convert;
mod world;

pub use world::PhysicsWorld;

use crate::core::scene::Scene;
use glam::Vec3;

#[derive(Copy, Clone, Debug)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    /// Evaluates the ray at parameter t
    pub fn point_at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}

/// Tests intersection of a Ray with an Axis-Aligned Bounding Box (AABB).
/// Returns Some(t) where t is the intersection distance along the ray, or None.
pub fn ray_aabb_intersection(ray: &Ray, min: Vec3, max: Vec3) -> Option<f32> {
    let mut t_min = f32::MIN;
    let mut t_max = f32::MAX;

    // Check X axis
    if ray.direction.x.abs() > f32::EPSILON {
        let tx1 = (min.x - ray.origin.x) / ray.direction.x;
        let tx2 = (max.x - ray.origin.x) / ray.direction.x;
        t_min = t_min.max(tx1.min(tx2));
        t_max = t_max.min(tx1.max(tx2));
    } else if ray.origin.x < min.x || ray.origin.x > max.x {
        return None;
    }

    // Check Y axis
    if ray.direction.y.abs() > f32::EPSILON {
        let ty1 = (min.y - ray.origin.y) / ray.direction.y;
        let ty2 = (max.y - ray.origin.y) / ray.direction.y;
        t_min = t_min.max(ty1.min(ty2));
        t_max = t_max.min(ty1.max(ty2));
    } else if ray.origin.y < min.y || ray.origin.y > max.y {
        return None;
    }

    // Check Z axis
    if ray.direction.z.abs() > f32::EPSILON {
        let tz1 = (min.z - ray.origin.z) / ray.direction.z;
        let tz2 = (max.z - ray.origin.z) / ray.direction.z;
        t_min = t_min.max(tz1.min(tz2));
        t_max = t_max.min(tz1.max(tz2));
    } else if ray.origin.z < min.z || ray.origin.z > max.z {
        return None;
    }

    // If near intersection is further than far, or far is behind ray, no intersection
    if t_min > t_max || t_max < 0.0 {
        return None;
    }

    // If t_min is negative, the ray origin is inside the box, so we return 0.0 (or t_min)
    if t_min < 0.0 {
        Some(0.0)
    } else {
        Some(t_min)
    }
}

/// Casts a ray into the scene and finds the closest intersecting entity with an
/// active collider. Returns `Some((entity_id, distance_t))`.
///
/// Used by the Lua `Physics.Raycast`/`Shoot` bindings, which hold only a `Scene`.
/// The in-engine hitscan uses `PhysicsWorld::cast_ray` (parry) instead.
pub fn cast_ray_in_scene(ray: &Ray, scene: &Scene) -> Option<(u32, f32)> {
    let mut closest_entity = None;
    let mut min_t = f32::MAX;

    for entity in scene.iter() {
        if !entity.active {
            continue;
        }

        // Only test against active colliders (excluding the Player itself to avoid self-hits)
        if let Some(collider) = &entity.collider {
            if !collider.active || entity.name == "Player" {
                continue;
            }

            // Exclude dead entities
            if let Some(health) = &entity.health {
                if health.is_dead {
                    continue;
                }
            }

            // Perform ray-AABB test
            if let Some(t) = ray_aabb_intersection(ray, collider.aabb_min, collider.aabb_max) {
                if t < min_t {
                    min_t = t;
                    closest_entity = Some(entity.id);
                }
            }
        }
    }

    closest_entity.map(|id| (id, min_t))
}
