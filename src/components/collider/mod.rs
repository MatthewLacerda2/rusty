//! src/components/collider/mod.rs — Collider component
//!
//! box/sphere/cylinder/capsule/mesh shapes, the inline physics material
//! (`physics_material`), and the cached world AABB. Unity: Collider.

mod physics_material;

pub use physics_material::{CombineMode, PhysicsMaterial};

use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

/// The local axis a capsule's length runs along (Unity: `CapsuleCollider.direction`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapsuleAxis {
    X,
    #[default]
    Y,
    Z,
}

impl CapsuleAxis {
    /// The name the Lua API and inspector show (`"X"` / `"Y"` / `"Z"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
        }
    }

    /// Parse the Lua/editor name back to the axis, `None` on an unknown string.
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.as_str() == s)
    }

    pub const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];

    /// The unit vector of this axis in the collider's local space.
    pub fn unit(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
        }
    }
}

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
    /// A capsule (#447): a cylinder capped by two hemispheres, the standard shape
    /// for characters and limb hitboxes. As in Unity, `height` is the **full**
    /// end-to-end length, caps included; a height below `2 * radius` degenerates
    /// to a sphere of `radius`.
    Capsule {
        radius: f32,
        height: f32,
        axis: CapsuleAxis,
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
    /// Friction, bounciness and their combine modes (#447). Absent in scenes
    /// saved before it, which load with the defaults.
    #[serde(default)]
    pub material: PhysicsMaterial,
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
            material: PhysicsMaterial::default(),
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        }
    }

    pub fn calculate_world_aabb(&self, world_mat: Mat4) -> (Vec3, Vec3) {
        let local_corners = match &self.shape {
            ColliderShape::Box { size } => box_corners(*size * -0.5, *size * 0.5),
            ColliderShape::Sphere { radius } => {
                let r = *radius;
                vec![Vec3::new(-r, -r, -r), Vec3::new(r, r, r)]
            }
            ColliderShape::Cylinder { radius, height } => {
                let r = *radius;
                let h = *height * 0.5;
                vec![Vec3::new(-r, -h, -r), Vec3::new(r, h, r)]
            }
            ColliderShape::Capsule {
                radius,
                height,
                axis,
            } => {
                let half = capsule_local_half_extents(*radius, *height, *axis);
                box_corners(-half, half)
            }
            ColliderShape::Mesh {
                local_min,
                local_max,
                ..
            } => box_corners(*local_min, *local_max),
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

/// A capsule's local half-extents: `radius` across, and half its full length
/// (never less than the caps' `radius`) along `axis`.
fn capsule_local_half_extents(radius: f32, height: f32, axis: CapsuleAxis) -> Vec3 {
    let along = (height * 0.5).max(radius);
    Vec3::splat(radius) + axis.unit() * (along - radius)
}

/// The eight corners of the local box `min`..`max`.
fn box_corners(n: Vec3, x: Vec3) -> Vec<Vec3> {
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

    fn capsule(radius: f32, height: f32, axis: CapsuleAxis) -> ColliderComponent {
        ColliderComponent {
            shape: ColliderShape::Capsule {
                radius,
                height,
                axis,
            },
            ..ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO)
        }
    }

    fn assert_vec(got: Vec3, want: Vec3) {
        assert!((got - want).length() < 1e-4, "got {got}, want {want}");
    }

    #[test]
    fn capsule_world_aabb_spans_full_height_along_its_axis() {
        // Unity semantics: height 2 is the full end-to-end length, caps included.
        let c = capsule(0.5, 2.0, CapsuleAxis::Y);
        let (min, max) = c.calculate_world_aabb(Mat4::from_translation(Vec3::new(1.0, 3.0, 0.0)));
        assert_vec(min, Vec3::new(0.5, 2.0, -0.5));
        assert_vec(max, Vec3::new(1.5, 4.0, 0.5));

        let (min, max) = capsule(0.25, 3.0, CapsuleAxis::X).calculate_world_aabb(Mat4::IDENTITY);
        assert_vec(min, Vec3::new(-1.5, -0.25, -0.25));
        assert_vec(max, Vec3::new(1.5, 0.25, 0.25));

        let (min, max) = capsule(0.25, 3.0, CapsuleAxis::Z).calculate_world_aabb(Mat4::IDENTITY);
        assert_vec(min, Vec3::new(-0.25, -0.25, -1.5));
        assert_vec(max, Vec3::new(0.25, 0.25, 1.5));
    }

    #[test]
    fn short_capsule_bounds_are_its_sphere() {
        // height < 2r: the caps meet, the capsule is a sphere of radius r.
        let (min, max) = capsule(1.0, 0.5, CapsuleAxis::Y).calculate_world_aabb(Mat4::IDENTITY);
        assert_vec(min, Vec3::splat(-1.0));
        assert_vec(max, Vec3::splat(1.0));
    }

    #[test]
    fn rotated_capsule_bounds_follow_the_rotation() {
        // A Y capsule turned 90° about Z lies along X.
        let rot = Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let (min, max) = capsule(0.5, 4.0, CapsuleAxis::Y).calculate_world_aabb(rot);
        assert_vec(min, Vec3::new(-2.0, -0.5, -0.5));
        assert_vec(max, Vec3::new(2.0, 0.5, 0.5));
    }

    #[test]
    fn capsule_axis_names_round_trip() {
        for a in CapsuleAxis::ALL {
            assert_eq!(CapsuleAxis::parse(a.as_str()), Some(a));
        }
        assert_eq!(CapsuleAxis::parse("y"), None);
        assert_eq!(CapsuleAxis::default(), CapsuleAxis::Y);
    }

    #[test]
    fn pre_447_collider_json_loads_with_default_material() {
        let json = r#"{"active":true,"shape":{"Sphere":{"radius":1.0}},"is_trigger":false,
            "aabb_min":[0,0,0],"aabb_max":[0,0,0]}"#;
        let c: ColliderComponent = serde_json::from_str(json).unwrap();
        assert_eq!(c.material, PhysicsMaterial::default());
    }
}
