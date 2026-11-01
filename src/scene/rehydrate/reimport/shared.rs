//! src/scene/rehydrate/reimport/shared.rs — a live re-import reaches a Mesh-shaped
//! collider's shared mesh (#1657) too: a part whose Mesh was removed collides with,
//! and feeds the navmesh, the model as it is now, not as it was at the last load.

use std::collections::BTreeMap;
use std::path::Path;

use super::super::{bounds_of, install, RehydratedMesh};
use super::refit;
use crate::asset::{self, AssetRef, Imported};
use crate::components::ColliderShape;
use crate::scene::prefab::diff::masked;
use crate::scene::prefab::live::shift_baseline;
use crate::scene::Scene;

/// Rebuild every Mesh-shaped collider's shared mesh, on an entity without a Mesh,
/// whose reference's file `names` accepts, from `imported` (each file imported
/// once, shared with the Meshes' walk), and refit its collider's box. A sub-object
/// that now imports empty keeps the old geometry, as a Mesh does, so the part
/// never drops out of physics.
pub(super) fn reimport_shared(
    scene: &mut Scene,
    names: &mut impl FnMut(&str) -> bool,
    imported: &mut BTreeMap<String, Option<Imported>>,
) {
    for id in scene.world.ids_with_collider() {
        if scene.world.has_mesh(id) {
            continue;
        }
        let Some(reference) = scene.world.collider(id).and_then(|c| {
            if !matches!(c.shape, ColliderShape::Mesh { .. }) {
                return None;
            }
            let shared = c.shared_mesh.as_ref()?;
            AssetRef::parse(shared.asset_ref.as_deref()?).ok()
        }) else {
            continue;
        };
        if !names(&reference.path) {
            continue;
        }
        let asset = imported
            .entry(reference.path)
            .or_insert_with_key(|file| asset::imported(Path::new(file)).ok());
        let Some((sub, revision)) = asset
            .as_ref()
            .and_then(|a| Some((a.sub_mesh(&reference.sub_object)?, a.revision)))
        else {
            continue;
        };
        // A linked prefab entity's refit is the model's, never an override (#1527).
        let linked = scene.world.prefab_link(id).is_some();
        let before = linked.then(|| scene.world.reference_document(id)).flatten();
        let Some(mut collider) = scene.world.collider_mut(id) else {
            continue;
        };
        let Some(shared) = collider.shared_mesh.as_mut() else {
            continue;
        };
        let mut mesh = shared.reference();
        install(&mut mesh, RehydratedMesh::from_sub_mesh(sub, revision));
        let Some(bounds) = bounds_of(mesh.vertices.iter().map(|v| v.position)) else {
            continue;
        };
        shared.take_geometry(&mesh);
        refit(&mut collider, bounds);
        drop(collider);
        if let Some(before) = before {
            shift_baseline(&mut scene.world, id, &masked(&before));
        }
    }
}
