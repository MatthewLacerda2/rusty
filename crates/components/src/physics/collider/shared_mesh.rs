//! A Mesh-shaped collider's own mesh (Unity's `MeshCollider.sharedMesh`): the copy kept when its
//! Mesh is removed, or its collision mesh.

use serde::{Deserialize, Serialize};

use crate::mesh::MeshComponent;
use rusty_asset::mesh_data::Shared;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SharedMesh {
    pub primitive_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_ref: Option<String>,
    /// Starts at 1, as `MeshComponent`'s does: change the two together.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub subdivisions: u32,
    #[serde(skip)]
    pub positions: Vec<[f32; 3]>,
    #[serde(skip)]
    pub indices: Shared<u32>,
}

fn one() -> u32 {
    1
}

impl Default for SharedMesh {
    fn default() -> Self {
        Self {
            primitive_type: String::new(),
            asset_ref: None,
            subdivisions: one(),
            positions: Vec::new(),
            indices: Shared::default(),
        }
    }
}

fn is_one(n: &u32) -> bool {
    *n == 1
}

impl SharedMesh {
    pub fn of(mesh: &MeshComponent) -> Self {
        Self {
            primitive_type: mesh.primitive_type.clone(),
            asset_ref: mesh.asset_ref.clone(),
            subdivisions: mesh.subdivisions,
            positions: mesh.vertices.iter().map(|v| v.position).collect(),
            indices: mesh.indices.clone(),
        }
    }

    pub fn reference(&self) -> MeshComponent {
        MeshComponent {
            asset_ref: self.asset_ref.clone(),
            subdivisions: self.subdivisions,
            ..MeshComponent::new(self.primitive_type.clone(), Vec::new(), Vec::new())
        }
    }

    pub fn take_geometry(&mut self, mesh: &MeshComponent) {
        self.positions = mesh.vertices.iter().map(|v| v.position).collect();
        self.indices = mesh.indices.clone();
    }

    /// What an undo record and a prefab diff keep, as [`MeshComponent::without_geometry`] does.
    pub fn without_geometry(&self) -> Self {
        Self {
            primitive_type: self.primitive_type.clone(),
            asset_ref: self.asset_ref.clone(),
            subdivisions: self.subdivisions,
            positions: Vec::new(),
            indices: Shared::default(),
        }
    }

    pub fn same_source(&self, other: &Self) -> bool {
        self.primitive_type == other.primitive_type
            && self.asset_ref == other.asset_ref
            && self.subdivisions == other.subdivisions
    }

    pub fn adopt_geometry(&mut self, from: &mut Self) {
        self.positions = std::mem::take(&mut from.positions);
        self.indices = std::mem::take(&mut from.indices);
    }
}
