mod physics_material;
mod scaled;
mod shared_mesh;

pub use physics_material::{CombineMode, PhysicsMaterial};
pub use scaled::{capsule_dims, round_radius, ScaledShape};
pub use shared_mesh::SharedMesh;

use glam::Vec3;

use crate::Pose;
use serde::{Deserialize, Serialize};

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum CapsuleAxis {
        X = "X",
        #[default]
        Y = "Y",
        Z = "Z",
    }
}

impl CapsuleAxis {
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
    Capsule {
        radius: f32,
        height: f32,
        axis: CapsuleAxis,
    },
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
    #[serde(default)]
    pub material: PhysicsMaterial,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_asset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_mesh: Option<SharedMesh>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collision_mesh: Option<SharedMesh>,
    #[serde(skip)]
    pub aabb_min: Vec3,
    #[serde(skip)]
    pub aabb_max: Vec3,
}

impl ColliderComponent {
    pub fn without_geometry(&self) -> Self {
        Self {
            active: self.active,
            shape: self.shape.clone(),
            is_trigger: self.is_trigger,
            material: self.material,
            material_asset: self.material_asset.clone(),
            shared_mesh: self.shared_mesh.as_ref().map(SharedMesh::without_geometry),
            collision_mesh: self
                .collision_mesh
                .as_ref()
                .map(SharedMesh::without_geometry),
            aabb_min: self.aabb_min,
            aabb_max: self.aabb_max,
        }
    }

    pub fn built_collision_mesh(&self) -> Option<&SharedMesh> {
        let mesh = self.collision_mesh.as_ref();
        mesh.filter(|m| !m.indices.is_empty() && matches!(self.shape, ColliderShape::Mesh { .. }))
    }

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
            material_asset: None,
            shared_mesh: None,
            collision_mesh: None,
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        }
    }

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

impl crate::FirstClass for ColliderComponent {
    const NAME: &'static str = "Collider";
}

#[cfg(test)]
mod tests;
