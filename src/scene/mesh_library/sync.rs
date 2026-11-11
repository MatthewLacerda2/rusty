//! Bring the entities drawing a library mesh to its entry (`Scene::sync_scene_meshes`).

use std::collections::{BTreeMap, BTreeSet};

use super::scene_mesh_name;
use crate::components::mesh::{MeshRevision, Vertex, SCENE_KIND};
use crate::components::ColliderShape;
use crate::scene::rehydrate::{bounds_of, refit_collider, warn_once_of};
use crate::scene::Scene;

/// The names of the entries some entity is behind on, re-minting an entry's
/// revision when an entity carries a newer one (its data went back, an Undo), so
/// the renderer, which keeps the newest upload, takes the older data again. A name
/// with no entry is warned of once and left as it is.
pub(super) fn stale_entries(scene: &mut Scene) -> BTreeSet<String> {
    let mut changed = BTreeSet::new();
    for id in scene.world.ids().to_vec() {
        let Some(mesh) = scene.world.mesh(id) else {
            continue;
        };
        let Some(name) = scene_mesh_name(&mesh) else {
            continue;
        };
        let Some(entry) = scene.meshes.get_mut(name) else {
            warn_once_of(&format!("{SCENE_KIND}::{name}"), || {
                format!("entity {id} draws the mesh {name:?}, which this scene does not define")
            });
            continue;
        };
        if mesh.revision != entry.revision {
            if mesh.revision > entry.revision {
                entry.revision = MeshRevision::next();
            }
            changed.insert(name.to_string());
        }
    }
    changed
}

/// Give every entity drawing a `changed` entry its geometry and revision, and fill
/// each Mesh collider's kept copy of a library mesh that is empty or `changed`.
pub(super) fn install(scene: &mut Scene, changed: &BTreeSet<String>) {
    let mut built: BTreeMap<String, (Vec<Vertex>, Vec<u32>)> = BTreeMap::new();
    let mut geometry = |scene: &Scene, name: &str| {
        let entry = &scene.meshes[name];
        let g = built
            .entry(name.to_string())
            .or_insert_with(|| entry.data.geometry());
        (g.clone(), entry.revision)
    };
    for id in scene.world.ids().to_vec() {
        let name = scene
            .world
            .mesh(id)
            .and_then(|m| scene_mesh_name(&m).map(str::to_string));
        if let Some(name) = name.filter(|n| changed.contains(n)) {
            let ((vertices, indices), revision) = geometry(scene, &name);
            if let Some(mut mesh) = scene.world.mesh_mut(id) {
                mesh.vertices = vertices;
                mesh.indices = indices;
                mesh.ranges.clear();
                mesh.revision = revision;
                mesh.clusters = None;
            }
            refit(scene, id);
        }
        fill_kept_copy(scene, id, changed, &mut geometry);
    }
}

/// Fill `id`'s Mesh collider's kept copy (#1657) when it names a library mesh and
/// is empty or `changed`.
fn fill_kept_copy(
    scene: &mut Scene,
    id: u32,
    changed: &BTreeSet<String>,
    geometry: &mut impl FnMut(&Scene, &str) -> ((Vec<Vertex>, Vec<u32>), MeshRevision),
) {
    let name = scene.world.collider(id).and_then(|c| {
        let kept = c.shared_mesh.as_ref()?;
        let named = kept.primitive_type == SCENE_KIND;
        let due = kept.positions.is_empty()
            || kept.asset_ref.as_ref().is_some_and(|n| changed.contains(n));
        kept.asset_ref
            .clone()
            .filter(|n| named && due && scene.meshes.contains_key(n))
    });
    let Some(name) = name else {
        return;
    };
    let ((vertices, indices), _) = geometry(scene, &name);
    if let Some(mut collider) = scene.world.collider_mut(id) {
        if let Some(kept) = collider.shared_mesh.as_mut() {
            kept.positions = vertices.iter().map(|v| v.position).collect();
            kept.indices = indices;
        }
    }
    refit(scene, id);
}

/// Size `id`'s Mesh collider's box to what it now builds from, and its world box.
pub(super) fn refit(scene: &mut Scene, id: u32) {
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
