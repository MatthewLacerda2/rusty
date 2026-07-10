//! src/scene/asset_instance.rs — Instantiate one imported sub-object (#182).
//!
//! The "drag a glTF from the content browser into the scene" verb, given an API
//! equivalent: spawn ONE entity from a path-based asset reference
//! (`path::sub_object`), carrying the importer-built mesh, its resolved material
//! (or the engine default), and a synced collider when the sidecar asks for one.
//!
//! Both callers route through here so editor and API can never drift: the editor's
//! model inspector ("Instantiate into Scene", one entity per sub-object) and the
//! `Scene.Instantiate` asset branch (one entity by reference). The shared piece is
//! [`instantiate_asset`]; the editor loops it over every sub-object of a file.
//!
//! Like every other mesh, an instanced asset stores only its REFERENCE on disk:
//! [`asset_mesh_component`](crate::scene::asset_mesh_component) keeps the
//! `path::sub_object` reference and the geometry is re-imported on scene load — no
//! GPU buffers are ever inlined, so save + reload preserves the reference exactly
//! as a primitive's `primitive_type` rehydrates.

use glam::Vec3;

use crate::asset::{self, ImportedAsset};
use crate::components::{ColliderComponent, MaterialComponent};
use crate::scene::authoring::material_asset_from_import;
use crate::scene::Scene;

/// Spawn one entity from a path-based asset reference (`path::sub_object`), placed
/// at `position`. Imports the source file, attaches the referenced sub-mesh (kept
/// as a reference, re-imported on load), resolves + applies its material (the
/// engine default when the sub-mesh names none), and syncs a collider when the
/// file's `.meta` sidecar records one for that sub-object. Returns the new id.
///
/// `name` defaults to the sub-object id when `None` — the same label the editor
/// gives a freshly-instanced sub-object. Errors with a human-readable string when
/// the reference is malformed, the file fails to import, or the sub-object is
/// absent — the same failures `import_sub_mesh` surfaces, but caught up front so a
/// bad reference never produces an empty-geometry phantom entity.
pub fn instantiate_asset(
    scene: &mut Scene,
    reference: &str,
    name: Option<&str>,
    position: Vec3,
) -> Result<u32, String> {
    let asset_ref = asset::AssetRef::parse(reference).map_err(|e| e.to_string())?;
    let asset = asset::import_and_sync_sidecar(std::path::Path::new(&asset_ref.path))
        .map_err(|e| e.to_string())?;
    if asset.sub_mesh(&asset_ref.sub_object).is_none() {
        return Err(format!(
            "asset reference '{reference}' has no such sub-object"
        ));
    }

    let label = name.unwrap_or(&asset_ref.sub_object);
    let id = scene.add_entity(label.to_string());
    if let Some(mut transform) = scene.world.transform_mut(id) {
        transform.position = position;
    }
    scene
        .world
        .set_mesh(id, Some(crate::scene::asset_mesh_component(reference)));

    apply_material(scene, id, &asset_ref.path, &asset, &asset_ref.sub_object);
    if sync_collider(scene, id, &asset_ref.path, &asset, &asset_ref.sub_object) {
        scene.update_all_colliders();
    }
    Ok(id)
}

/// Resolve the sub-mesh's imported material into the scene library and reference it
/// from `id`; when the sub-mesh names no material, dress the entity with the engine
/// default — a GameObject built from a glTF carries a Material by definition
/// (CLAUDE.md), so it renders with a resolvable reference rather than none.
fn apply_material(scene: &mut Scene, id: u32, path: &str, asset: &ImportedAsset, sub: &str) {
    let key = asset
        .sub_mesh(sub)
        .and_then(|s| s.material)
        .and_then(|idx| import_material(scene, path, asset, idx));
    match key {
        Some(key) => {
            scene
                .world
                .set_material(id, Some(MaterialComponent { material: key }));
        }
        None => {
            crate::scene::authoring::attach_default_material(scene, id);
        }
    }
}

/// Convert sub-mesh material `idx` into a `MaterialAsset`, insert it under a
/// DETERMINISTIC library key, and return that key — `"{path}::{name}"` for a named
/// glTF material, else `"{path}::material_{idx}"`, so sub-objects sharing one glTF
/// material share one library entry (dedup). `None` if the index is out of range.
fn import_material(
    scene: &mut Scene,
    path: &str,
    asset: &ImportedAsset,
    idx: usize,
) -> Option<String> {
    let data = asset.materials.get(idx)?;
    let key = if data.name.is_empty() {
        format!("{path}::material_{idx}")
    } else {
        format!("{path}::{}", data.name)
    };
    scene
        .materials
        .insert(key.clone(), material_asset_from_import(data));
    Some(key)
}

/// Attach a mesh collider to `id` when the file's sidecar records one for `sub`,
/// sized from the sub-mesh bounds (consistent with the editor's import path).
/// Returns `true` if a collider was added (so the caller refreshes world AABBs).
fn sync_collider(scene: &mut Scene, id: u32, path: &str, asset: &ImportedAsset, sub: &str) -> bool {
    let settings = asset::sidecar::load(std::path::Path::new(path)).unwrap_or_default();
    let Some(kind) = settings.colliders.get(sub).copied() else {
        return false;
    };
    let Some((min, max)) = asset.sub_mesh(sub).and_then(|s| s.bounds()) else {
        return false;
    };
    scene.world.set_collider(
        id,
        Some(ColliderComponent::from_mesh_bounds(
            kind.is_convex(),
            min,
            max,
        )),
    )
}

#[cfg(test)]
#[path = "asset_instance_tests.rs"]
mod asset_instance_tests;
