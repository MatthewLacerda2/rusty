//! src/components/physics/collider/mod.rs — Collider component
//!
//! box/sphere/cylinder/capsule/mesh shapes, the inline physics material
//! (`physics_material`), the mesh it keeps once its Mesh is gone (`shared_mesh`),
//! the shape under a world scale (`scaled`), and the
//! cached world AABB. Unity: Collider.

mod physics_material;
mod scaled;
mod shared_mesh;

pub use physics_material::{CombineMode, PhysicsMaterial};
pub use scaled::{capsule_dims, round_radius, ScaledShape};
pub use shared_mesh::SharedMesh;

use glam::Vec3;

use crate::components::Pose;
use serde::{Deserialize, Serialize};

crate::core::named_enum! {
    /// The local axis a capsule's length runs along (Unity: `CapsuleCollider.direction`).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum CapsuleAxis {
        X = "X",
        #[default]
        Y = "Y",
        Z = "Z",
    }
}

impl CapsuleAxis {
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColliderComponent {
    pub active: bool,
    pub shape: ColliderShape,
    pub is_trigger: bool,
    /// Friction, bounciness and their combine modes (#447). Absent in scenes
    /// saved before it, which load with the defaults.
    #[serde(default)]
    pub material: PhysicsMaterial,
    /// The mesh a `Mesh` shape builds from once its entity has no Mesh (#1657):
    /// Unity's `MeshCollider.sharedMesh`. Set when the Mesh is removed, cleared when
    /// one is added; the entity's own Mesh wins while it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_mesh: Option<SharedMesh>,
    /// The world-space bounds, derived from the shape and the entity's world pose
    /// (`ecs::world_space::bounds`): never saved, and recomputed on every load.
    #[serde(skip)]
    pub aabb_min: Vec3,
    #[serde(skip)]
    pub aabb_max: Vec3,
}

impl ColliderComponent {
    /// This collider with its shared mesh's geometry left out
    /// ([`SharedMesh::without_geometry`]): what an undo record and a prefab diff copy.
    /// Every field is named, so a new one must be placed here.
    pub fn without_geometry(&self) -> Self {
        Self {
            active: self.active,
            shape: self.shape.clone(),
            is_trigger: self.is_trigger,
            material: self.material.clone(),
            shared_mesh: self.shared_mesh.as_ref().map(SharedMesh::without_geometry),
            aabb_min: self.aabb_min,
            aabb_max: self.aabb_max,
        }
    }

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
            shared_mesh: None,
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        }
    }

    /// The world AABB of the shape physics builds at `pose` (a world
    /// [`Pose`], as `World::world_pose` gives it): the shape sized by
    /// [`ColliderShape::scaled`], as `physics::shape::build` sizes it, then turned and
    /// moved by the pose. A sphere, cylinder or capsule stays round under
    /// non-uniform scale, and a mesh's local box is carried through the pose's
    /// signed scale (#1140).
    pub fn calculate_world_aabb(&self, pose: Pose) -> (Vec3, Vec3) {
        let (n, x) = self.shape.scaled(pose.scale).local_box();
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for corner in box_corners(n, x) {
            let world_pos = pose.pos + pose.rot * corner;
            min = min.min(world_pos);
            max = max.max(world_pos);
        }
        (min, max)
    }
}

/// The eight corners of the local box `min`..`max`.
fn box_corners(n: Vec3, x: Vec3) -> [Vec3; 8] {
    [
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
