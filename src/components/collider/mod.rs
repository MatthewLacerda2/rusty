//! src/components/collider/mod.rs — Collider component
//!
//! box/sphere/cylinder/capsule/mesh shapes, the inline physics material
//! (`physics_material`), and the cached world AABB. Unity: Collider.

mod physics_material;

pub use physics_material::{CombineMode, PhysicsMaterial};

use glam::{Mat4, Quat, Vec3};
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

    /// The world AABB of the shape physics builds under `world_mat`. A box or
    /// mesh is its local box carried through the matrix. A sphere, cylinder or
    /// capsule stays round under non-uniform scale, sized by [`round_radius`] as
    /// `physics::build` sizes it (#1140), so its box is the scaled shape's box,
    /// turned and moved by the matrix's rotation and translation.
    pub fn calculate_world_aabb(&self, world_mat: Mat4) -> (Vec3, Vec3) {
        let (scale, rot, pos) = world_mat.to_scale_rotation_translation();
        // A zero scale leaves no rotation to read; the flat shape needs none.
        let rot = if rot.is_finite() { rot } else { Quat::IDENTITY };
        let s = scale.abs();
        let round = |half: Vec3| {
            (
                box_corners(-half, half),
                Mat4::from_rotation_translation(rot, pos),
            )
        };
        let (local_corners, mat) = match &self.shape {
            ColliderShape::Box { size } => (box_corners(*size * -0.5, *size * 0.5), world_mat),
            ColliderShape::Sphere { radius } => round(Vec3::splat(*radius * s.max_element())),
            ColliderShape::Cylinder { radius, height } => {
                let r = round_radius(*radius, s, CapsuleAxis::Y);
                round(Vec3::new(r, *height * s.y * 0.5, r))
            }
            ColliderShape::Capsule {
                radius,
                height,
                axis,
            } => round(capsule_local_half_extents(
                round_radius(*radius, s, *axis),
                *height * s.dot(axis.unit()),
                *axis,
            )),
            ColliderShape::Mesh {
                local_min,
                local_max,
                ..
            } => (box_corners(*local_min, *local_max), world_mat),
        };

        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);

        for &corner in &local_corners {
            let world_pos = mat.transform_point3(corner);
            min = min.min(world_pos);
            max = max.max(world_pos);
        }

        (min, max)
    }
}

/// A round cross-section's radius under `scale`: `radius` times the larger of
/// the two scales across `axis`, so the shape never sits inside its drawn
/// surface. The one rule a cylinder (axis Y) and a capsule share, in physics
/// and in the AABB (#1131, #1140). A mirrored axis keeps its size.
pub fn round_radius(radius: f32, scale: Vec3, axis: CapsuleAxis) -> f32 {
    let s = scale.abs();
    let cross = match axis {
        CapsuleAxis::X => s.y.max(s.z),
        CapsuleAxis::Y => s.x.max(s.z),
        CapsuleAxis::Z => s.x.max(s.y),
    };
    radius * cross
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
mod tests;
