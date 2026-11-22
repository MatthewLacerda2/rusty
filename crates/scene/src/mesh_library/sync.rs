use std::collections::BTreeSet;

use super::{kept_mesh_name, scene_mesh_name, scene_mesh_users};
use crate::rehydrate::{bounds_of, refit_collider, warn_once_of};
use crate::Scene;
use rusty_components::mesh::MeshRevision;
use rusty_components::ColliderShape;

pub(super) fn stale_entries(scene: &mut Scene) -> BTreeSet<String> {
    let mut changed = BTreeSet::new();
    for id in scene.world.ids().to_vec() {
        let (mesh, collider) = (scene.world.mesh(id), scene.world.collider(id));
        let names: Vec<String> = scene_mesh_users(mesh.as_deref(), collider.as_deref())
            .map(str::to_string)
            .collect();
        for name in names {
            let Some(entry) = scene.meshes.get_mut(&name) else {
                warn_once_of(&super::missing_key(&name), || {
                    format!("entity {id} draws the mesh {name:?}, which this scene does not define")
                });
                continue;
            };
            let own = mesh
                .as_deref()
                .filter(|m| scene_mesh_name(m) == Some(&name));
            if let Some(mesh) = own.filter(|m| m.revision != entry.revision) {
                if mesh.revision > entry.revision {
                    entry.revision = MeshRevision::next();
                }
                changed.insert(name.clone());
            }
            let kept = collider.as_deref().and_then(|c| c.shared_mesh.as_ref());
            let kept = kept.filter(|_| collider.as_deref().and_then(kept_mesh_name) == Some(&name));
            if kept.is_some_and(|k| {
                k.positions != entry.data.positions || k.indices != entry.data.indices
            }) {
                changed.insert(name);
            }
        }
    }
    changed
}

pub(super) fn install(scene: &mut Scene, changed: &BTreeSet<String>) {
    for id in scene.world.ids().to_vec() {
        let name = scene
            .world
            .mesh(id)
            .and_then(|m| scene_mesh_name(&m).map(str::to_string));
        if let Some(entry) = name
            .filter(|n| changed.contains(n))
            .map(|n| scene.meshes[&n].clone())
        {
            let (vertices, indices) = entry.geometry().clone();
            if let Some(mut mesh) = scene.world.mesh_mut(id) {
                mesh.vertices = vertices;
                mesh.indices = indices;
                mesh.ranges.clear();
                mesh.revision = entry.revision;
                mesh.own_geometry = None;
                mesh.clusters = None;
            }
            refit(scene, id);
        }
        fill_kept_copy(scene, id, changed);
    }
}

fn fill_kept_copy(scene: &mut Scene, id: u32, changed: &BTreeSet<String>) {
    let name = scene
        .world
        .collider(id)
        .and_then(|c| kept_mesh_name(&c).map(str::to_string));
    let Some(entry) = name
        .filter(|n| changed.contains(n))
        .map(|n| scene.meshes[&n].clone())
    else {
        return;
    };
    if let Some(mut collider) = scene.world.collider_mut(id) {
        if let Some(kept) = collider.shared_mesh.as_mut() {
            kept.positions = entry.data.positions.clone();
            kept.indices = entry.indices();
        }
    }
    refit(scene, id);
}

pub fn refit(scene: &mut Scene, id: u32) {
    let own = scene
        .world
        .mesh(id)
        .and_then(|m| bounds_of(m.vertices.iter().map(|v| v.position)));
    if let Some(mut collider) = scene.world.collider_mut(id) {
        if matches!(collider.shape, ColliderShape::Mesh { .. }) {
            refit_collider(&mut collider, own);
        }
    }
    scene.world.refresh_collider_aabb(id);
}
