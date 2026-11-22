//! Editor and API both route through here. Like every mesh, an instanced asset stores only its reference; the geometry is re-imported on load.

use glam::Vec3;

use crate::authoring::material_asset_from_import;
use crate::lod_instance::instantiate_lod_set;
use crate::rehydrate::{self as reimports, Rig};
use rusty_asset::{Imported, ImportedAsset};
use rusty_components::{ColliderComponent, MaterialComponent};
use rusty_scene::rehydrate;
use rusty_scene::Scene;

/// A sub-object name that is the base of a `_LOD<n>` set (`crate.glb::Crate` for `Crate_LOD0`, `Crate_LOD1`) spawns the whole set as one LODGroup; its id is the group's.
/// Errors up front on a bad reference, so it never leaves an empty-geometry entity.
pub fn instantiate_asset(
    scene: &mut Scene,
    reference: &str,
    name: Option<&str>,
    position: Vec3,
) -> Result<u32, String> {
    if !reference.contains(rusty_asset::REF_SEPARATOR) && rusty_asset::is_model_path(reference) {
        let asset = rusty_asset::import_and_sync_sidecar(std::path::Path::new(reference))
            .map_err(|e| e.to_string())?;
        let root = crate::model_instance::instantiate_model(scene, reference, &asset, position);
        if let Some(name) = name {
            scene.world.set_name(root, name.to_string());
        }
        return Ok(root);
    }
    let asset_ref = rusty_asset::AssetRef::parse(reference).map_err(|e| e.to_string())?;
    let asset = rusty_asset::import_and_sync_sidecar(std::path::Path::new(&asset_ref.path))
        .map_err(|e| e.to_string())?;
    let (path, sub) = (&asset_ref.path, &asset_ref.sub_object);
    instantiate_imported(scene, path, &asset, sub, name, position)
}

pub fn instantiate_imported(
    scene: &mut Scene,
    path: &str,
    asset: &Imported,
    sub: &str,
    name: Option<&str>,
    position: Vec3,
) -> Result<u32, String> {
    let reference = rusty_asset::AssetRef::new(path, sub)
        .map_err(|e| e.to_string())?
        .to_string_ref();
    let Some(sub_mesh) = asset.sub_mesh(sub) else {
        return match rusty_asset::lod::lod_set(asset, sub) {
            Some(set) => instantiate_lod_set(scene, path, asset, &set, name, position),
            None => Err(format!(
                "asset reference '{reference}' has no such sub-object"
            )),
        };
    };

    let label = name.unwrap_or(sub);
    let id = scene.add_entity(label.to_string());
    if let Some(mut transform) = scene.world.transform_mut(id) {
        transform.position = position;
    }
    scene.world.set_mesh(
        id,
        Some(rehydrate::asset_mesh_component_of(
            &reference,
            sub_mesh,
            asset.revision,
        )),
    );
    scene.sync_skeleton(id);

    apply_material(scene, id, path, asset, sub);
    if sync_collider(scene, id, path, asset, sub) {
        scene.update_all_colliders();
    }
    Ok(id)
}

fn apply_material(scene: &mut Scene, id: u32, path: &str, asset: &ImportedAsset, sub: &str) {
    let runs: Vec<Option<usize>> = asset
        .sub_mesh(sub)
        .map(|s| s.ranges.iter().map(|r| r.material).collect())
        .unwrap_or_default();
    let mut imported: Vec<(usize, Option<String>)> = Vec::new();
    let mut key_of = |idx: usize| {
        if let Some((_, key)) = imported.iter().find(|(i, _)| *i == idx) {
            return key.clone();
        }
        let key = import_material(scene, path, asset, idx);
        imported.push((idx, key.clone()));
        key
    };
    let keys: Vec<Option<String>> = runs.iter().map(|run| run.and_then(&mut key_of)).collect();
    if keys.is_empty() || keys.iter().any(Option::is_none) {
        crate::authoring::attach_default_material(scene, id);
    }
    if keys.iter().all(Option::is_none) {
        return;
    }
    let fallback = rusty_components::material::entity_material_key(id);
    let materials = keys
        .into_iter()
        .map(|k| k.unwrap_or_else(|| fallback.clone()))
        .collect();
    scene
        .world
        .set_material(id, Some(MaterialComponent { materials }));
}

/// Key `"{path}::{name}"` for a named glTF material, else `"{path}::material_{idx}"`, so sub-objects sharing one material share one entry.
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
    // A material already in the scene is the user's (a placed model's recolour): a later spawn of the same file must not reset it.
    scene
        .materials
        .entry(key.clone())
        .or_insert_with(|| material_asset_from_import(data));
    Some(key)
}

fn sync_collider(scene: &mut Scene, id: u32, path: &str, asset: &ImportedAsset, sub: &str) -> bool {
    let settings = rusty_asset::sidecar::load(std::path::Path::new(path)).unwrap_or_default();
    let Some(entry) = settings.colliders.get(sub) else {
        return false;
    };
    let mut collider =
        ColliderComponent::from_mesh_bounds(entry.kind.is_convex(), Vec3::ZERO, Vec3::ZERO);
    collider.collision_mesh =
        (entry.mesh.as_deref()).map(|name| collision::collision_mesh(path, asset, name));
    let own = asset.sub_mesh(sub).and_then(|s| s.bounds());
    if own.is_none() && collider.built_collision_mesh().is_none() {
        return false;
    }
    rehydrate::refit_collider(&mut collider, own);
    scene.world.set_collider(id, Some(collider))
}

pub fn collision_sub_objects(path: &str) -> std::collections::BTreeSet<String> {
    let settings = rusty_asset::sidecar::load(std::path::Path::new(path)).unwrap_or_default();
    let named = settings.colliders.into_values().filter_map(|c| c.mesh);
    named.collect()
}

pub mod collision;
mod humanoid;

pub use humanoid::set_humanoid;

#[doc(hidden)]
pub mod humanoid_fixture;
#[cfg(test)]
mod humanoid_tests;
#[cfg(test)]
mod multi_material_tests;
#[cfg(test)]
mod separator_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod texture_tests;

pub fn set_lightmap_uv_settings(
    scene: &mut Scene,
    path: &str,
    settings: rusty_asset::LightmapUvSettings,
) -> Result<usize, String> {
    rusty_asset::sidecar::set_lightmap_uvs(std::path::Path::new(path), settings)
        .map_err(|e| e.to_string())?;
    Ok(reimports::reimport_model(scene, path, Rig::Keep))
}

/// A NaN or unchanged factor changes nothing: no write, no re-import. The write log notes the model, so caches keyed by it re-read it.
pub fn set_model_scale_factor(scene: &mut Scene, path: &str, scale: f32) -> Result<usize, String> {
    let source = std::path::Path::new(path);
    let stored = rusty_asset::sidecar::load(source)
        .map_err(|e| e.to_string())?
        .scale;
    if rusty_asset::scale_factor::sanitize(scale, stored) == stored {
        return Ok(0);
    }
    rusty_asset::sidecar::set_scale(source, scale).map_err(|e| e.to_string())?;
    // A whole-model instance is rebuilt from the scaled import, so the refit skips its static parts.
    let refit = |scene: &mut Scene, skip: &_| {
        let before = crate::lod_instance::lod0_extents(scene);
        let ids = reimports::model_writes::reimport_noted_except(scene, path, Rig::Rebuild, skip);
        crate::lod_instance::rescale_groups_over(scene, &ids, &before);
        ids.len() + skip.len()
    };
    // The same match the write-log follow uses: a link naming the model by another spelling (absolute vs project-relative) is found too.
    let model = rusty_core::fs::written::Stale::new(vec![path.to_string()]);
    Ok(crate::prefab::restamp::restamp_models(
        scene,
        |s| model.matches(s),
        refit,
    ))
}

#[cfg(test)]
mod prefab_scale_tests;
#[cfg(test)]
mod scale_tests;

#[cfg(test)]
mod lightmap_tests;
