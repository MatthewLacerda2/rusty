//! Mesh trees: kept per scene, or built ahead in a load's warm-up and taken by the first
//! tree that places the mesh.

use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

use super::mesh::MeshTree;
use super::{RayTree, Resident};
use crate::debug_meshes::has_geometry;
use crate::scene::MeshHandle;
use crate::MeshId;
use rusty_components::mesh::MeshRevision;
use rusty_scene::Scene;

pub(crate) type Prebuilt = BTreeMap<(MeshId, MeshRevision), MeshTree>;

/// A static mesh's tree, built ahead (a load's warm-up).
pub(crate) fn prebuild(m: &rusty_components::MeshComponent) -> Option<MeshTree> {
    let geometry = has_geometry(&m.vertices, &m.indices);
    geometry.then(|| MeshTree::build(&m.vertices, &m.indices, m.index_ranges()))
}

pub(super) type Ahead<'a> = &'a mut dyn FnMut(MeshHandle, MeshRevision) -> Option<MeshTree>;

impl RayTree {
    /// `Some(true)` built here, `Some(false)` taken from `ahead`.
    pub(super) fn keep_mesh(
        &mut self,
        (id, mesh): (u32, MeshHandle),
        scene: &Scene,
        (resident, ahead): (Resident, Ahead),
    ) -> Option<bool> {
        let (Entry::Vacant(slot), Some(revision)) = (self.meshes.entry(mesh), resident(mesh))
        else {
            return None;
        };
        if let Some(tree) = ahead(mesh, revision) {
            slot.insert((revision, tree));
            return Some(false);
        }
        let m = scene.world.mesh(id)?;
        let tree = prebuild(&m)?;
        slot.insert((revision, tree));
        Some(true)
    }
}
