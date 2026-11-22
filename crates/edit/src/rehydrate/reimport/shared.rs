use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::prefab::diff::masked;
use crate::prefab::live::shift_baseline;
use rusty_asset::{AssetRef, Imported};
use rusty_components::ColliderShape;
use rusty_scene::rehydrate::{bounds_of, install_sub_mesh, refit_collider, warn_once_of, Which};
use rusty_scene::Scene;

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

fn reimport_one(
    scene: &mut Scene,
    id: u32,
    which: Which,
    names: &mut impl FnMut(&str) -> bool,
    imported: &mut BTreeMap<String, Option<Imported>>,
) {
    // Read-only: `collider_mut` would deep-copy the collider, and this runs twice per
    // collider on every re-import.
    let Some(reference) = scene.world.collider(id).and_then(|c| {
        if !matches!(c.shape, ColliderShape::Mesh { .. }) {
            return None;
        }
        let shared = which.get(&c).as_ref()?;
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
        .or_insert_with_key(|file| rusty_asset::imported(Path::new(file)).ok());
    let Some(asset) = asset.as_ref() else {
        return;
    };
    let sub = asset.sub_mesh(&reference.sub_object);
    let sub = sub.filter(|s| !s.indices.is_empty() && s.bounds().is_some());
    // A kept mesh whose sub-object imports empty keeps its old geometry, so the part never
    // drops out of physics; a collision mesh is emptied, so the collider falls back to its
    // entity's Mesh.
    if sub.is_none() && which == Which::Kept {
        return;
    }
    let own = scene.world.mesh(id).and_then(|m| mesh_bounds(&m));
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
        Some(sub) => install_sub_mesh(&mut mesh, sub, asset.revision),
        None => warn_gone(id, &format!("{file}::{}", reference.sub_object)),
    }
    shared.take_geometry(&mesh);
    refit_collider(&mut collider, own);
    drop(collider);
    // A linked prefab entity's refit is the model's change, never an override.
    if let Some(before) = before {
        shift_baseline(&mut scene.world, id, &masked(&before));
    }
}

fn mesh_bounds(mesh: &rusty_components::MeshComponent) -> Option<(glam::Vec3, glam::Vec3)> {
    bounds_of(mesh.vertices.iter().map(|v| v.position))
}

fn warn_gone(id: u32, reference: &str) {
    warn_once_of(reference, || {
        format!(
            "collision mesh '{reference}' is gone: entity {id}'s collider builds from its own mesh"
        )
    });
}
