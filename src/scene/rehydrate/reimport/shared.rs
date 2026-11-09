//! src/scene/rehydrate/reimport/shared.rs — a live re-import reaches a Mesh-shaped
//! collider's own meshes too: a part whose Mesh was removed (its kept `shared_mesh`,
//! #1657) and a part built from a collision mesh (`collision_mesh`, #1960) collide
//! with, and feed the navmesh, the model as it is now, not as it was at the last load.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::super::{bounds_of, install, refit_collider, warn_once_of, RehydratedMesh, Which};
use crate::asset::{self, AssetRef, Imported};
use crate::components::ColliderShape;
use crate::scene::prefab::diff::masked;
use crate::scene::prefab::live::shift_baseline;
use crate::scene::Scene;

/// Rebuild every Mesh-shaped collider's kept shared mesh (on an entity without a
/// Mesh) and collision mesh, outside `skip`, whose reference's file `names` accepts,
/// from `imported` (each file imported once, shared with the Meshes' walk), and refit
/// its collider's box. A kept mesh whose sub-object now imports empty keeps the old
/// geometry, as a Mesh does, so the part never drops out of physics; a collision
/// mesh whose sub-object is gone is emptied (warned once), so the collider falls
/// back to its entity's Mesh rather than to triangles the model no longer has.
pub(super) fn reimport_shared(
    scene: &mut Scene,
    skip: &BTreeSet<u32>,
    names: &mut impl FnMut(&str) -> bool,
    imported: &mut BTreeMap<String, Option<Imported>>,
) {
    for id in scene.world.ids_with_collider() {
        if skip.contains(&id) {
            continue;
        }
        if !scene.world.has_mesh(id) {
            reimport_one(scene, id, Which::Kept, names, imported);
        }
        reimport_one(scene, id, Which::Collision, names, imported);
    }
}

/// [`reimport_shared`] for entity `id`'s `which` mesh.
fn reimport_one(
    scene: &mut Scene,
    id: u32,
    which: Which,
    names: &mut impl FnMut(&str) -> bool,
    imported: &mut BTreeMap<String, Option<Imported>>,
) {
    let Some(reference) = scene.world.collider_mut(id).and_then(|mut c| {
        if !matches!(c.shape, ColliderShape::Mesh { .. }) {
            return None;
        }
        let shared = which.of(&mut c).as_ref()?;
        AssetRef::parse(shared.asset_ref.as_deref()?).ok()
    }) else {
        return;
    };
    if !names(&reference.path) {
        return;
    }
    let file = reference.path.clone();
    let asset = imported
        .entry(reference.path)
        .or_insert_with_key(|file| asset::imported(Path::new(file)).ok());
    // A file that fails to import keeps every mesh as it was, as the Meshes' walk does.
    let Some(asset) = asset.as_ref() else {
        return;
    };
    let sub = asset.sub_mesh(&reference.sub_object);
    let sub = sub.filter(|s| !s.indices.is_empty() && s.bounds().is_some());
    if sub.is_none() && which == Which::Kept {
        return;
    }
    let own = scene.world.mesh(id).and_then(|m| mesh_bounds(&m));
    // A linked prefab entity's refit is the model's, never an override (#1527).
    let linked = scene.world.prefab_link(id).is_some();
    let before = linked.then(|| scene.world.reference_document(id)).flatten();
    let Some(mut collider) = scene.world.collider_mut(id) else {
        return;
    };
    let Some(shared) = which.of(&mut collider).as_mut() else {
        return;
    };
    let mut mesh = shared.reference();
    match sub {
        Some(sub) => install(
            &mut mesh,
            RehydratedMesh::from_sub_mesh(sub, asset.revision),
        ),
        None => warn_gone(id, &format!("{file}::{}", reference.sub_object)),
    }
    shared.take_geometry(&mesh);
    refit_collider(&mut collider, own);
    drop(collider);
    if let Some(before) = before {
        shift_baseline(&mut scene.world, id, &masked(&before));
    }
}

/// The box around `mesh`'s rest-pose vertices.
fn mesh_bounds(mesh: &crate::components::MeshComponent) -> Option<(glam::Vec3, glam::Vec3)> {
    bounds_of(mesh.vertices.iter().map(|v| v.position))
}

/// Say once that the collision mesh `reference` is gone from its model.
fn warn_gone(id: u32, reference: &str) {
    warn_once_of(reference, || {
        format!(
            "collision mesh '{reference}' is gone: entity {id}'s collider builds from its own mesh"
        )
    });
}
