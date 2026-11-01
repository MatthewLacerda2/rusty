//! src/components/physics/collider/shared_mesh.rs — the mesh a Mesh-shaped collider
//! keeps of its own (#1657). Unity: `MeshCollider.sharedMesh`.
//!
//! A Mesh-shaped collider builds from its entity's Mesh. When that Mesh is removed
//! (`Scene.RemoveComponent(id, "Mesh")`: a blockout box kept as an invisible
//! collider), the collider keeps the Mesh's reference and rest-pose geometry here, so
//! physics and the navmesh still read it. Like a Mesh, only the reference is saved;
//! the geometry is rebuilt from it on load (`scene::rehydrate`), on a live re-import
//! of its model, and on an undo, whose record leaves it out as it leaves out a
//! Mesh's ([`SharedMesh::without_geometry`]). The entity's own Mesh, when it has
//! one, wins.

use serde::{Deserialize, Serialize};

use crate::components::mesh::MeshComponent;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SharedMesh {
    /// The removed Mesh's `primitive_type`: a primitive's name, or `"Asset"`.
    pub primitive_type: String,
    /// The removed Mesh's `asset_ref` (`path::sub_object`), for an `"Asset"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_ref: Option<String>,
    /// The removed Mesh's `subdivisions` (a `"Plane"`'s grid).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub subdivisions: u32,
    /// Rest-pose vertex positions, rebuilt from the reference, never saved.
    #[serde(skip)]
    pub positions: Vec<[f32; 3]>,
    /// Triangle indices into `positions`, rebuilt like them.
    #[serde(skip)]
    pub indices: Vec<u32>,
}

fn one() -> u32 {
    1
}

fn is_one(n: &u32) -> bool {
    *n == 1
}

impl SharedMesh {
    /// `mesh`'s reference and rest-pose geometry.
    pub fn of(mesh: &MeshComponent) -> Self {
        Self {
            primitive_type: mesh.primitive_type.clone(),
            asset_ref: mesh.asset_ref.clone(),
            subdivisions: mesh.subdivisions,
            positions: mesh.vertices.iter().map(|v| v.position).collect(),
            indices: mesh.indices.clone(),
        }
    }

    /// A Mesh carrying only this reference, no geometry: what rehydration rebuilds.
    pub fn reference(&self) -> MeshComponent {
        MeshComponent {
            asset_ref: self.asset_ref.clone(),
            subdivisions: self.subdivisions,
            ..MeshComponent::new(self.primitive_type.clone(), Vec::new(), Vec::new())
        }
    }

    /// Take `mesh`'s geometry (rebuilt from [`Self::reference`]).
    pub fn take_geometry(&mut self, mesh: &MeshComponent) {
        self.positions = mesh.vertices.iter().map(|v| v.position).collect();
        self.indices = mesh.indices.clone();
    }

    /// The reference alone, its geometry left empty: what an undo record and a
    /// prefab diff keep, as [`MeshComponent::without_geometry`] does for a Mesh.
    pub fn without_geometry(&self) -> Self {
        Self {
            primitive_type: self.primitive_type.clone(),
            asset_ref: self.asset_ref.clone(),
            subdivisions: self.subdivisions,
            positions: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Whether `other`'s geometry is this one's: the same primitive or asset, cut
    /// the same way ([`MeshComponent::same_source`]).
    pub fn same_source(&self, other: &Self) -> bool {
        self.primitive_type == other.primitive_type
            && self.asset_ref == other.asset_ref
            && self.subdivisions == other.subdivisions
    }

    /// Move `from`'s geometry onto this one ([`MeshComponent::adopt_geometry`]).
    pub fn adopt_geometry(&mut self, from: &mut Self) {
        self.positions = std::mem::take(&mut from.positions);
        self.indices = std::mem::take(&mut from.indices);
    }
}
