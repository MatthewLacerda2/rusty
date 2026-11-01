//! src/scene/asset_instance/ — Instantiate one imported sub-object (#182).
//!
//! The "drag a glTF from Unity's Project window into the scene" verb, given an API
//! equivalent: spawn ONE entity from a path-based asset reference
//! (`path::sub_object`), carrying the importer-built mesh, its resolved material
//! (or the engine default), and a synced collider when the sidecar asks for one.
//!
//! Both callers route through here so editor and API can never drift: the editor's
//! model inspector ("Instantiate into Scene") and the `Scene.Instantiate` asset
//! branch. The shared piece is [`instantiate_asset`]: a `path::sub_object`
//! reference spawns that one entity, and a bare model path the whole model as its
//! node tree (#1083, `scene::model_instance::instantiate_model`, Unity's model
//! prefab), as the editor's button does. [`instantiate_imported`] behind it serves
//! a caller that already holds the imported file: the whole-model spawn places
//! each node's mesh through it.
//!
//! Like every other mesh, an instanced asset stores only its REFERENCE on disk:
//! [`asset_mesh_component`](crate::scene::asset_mesh_component) keeps the
//! `path::sub_object` reference and the geometry is re-imported on scene load — no
//! GPU buffers are ever inlined, so save + reload preserves the reference exactly
//! as a primitive's `primitive_type` rehydrates.

use glam::Vec3;

use crate::asset::{self, Imported, ImportedAsset};
use crate::components::{ColliderComponent, MaterialComponent};
use crate::scene::authoring::material_asset_from_import;
use crate::scene::lod_instance::instantiate_lod_set;
use crate::scene::rehydrate::{self, Rig};
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
///
/// A sub-object name that is the base of a `_LOD<n>` set (`crate.glb::Crate` for
/// `Crate_LOD0`, `Crate_LOD1`) spawns the whole set as one LODGroup entity instead
/// (#472, `scene::lod_instance`); its id is the group's.
///
/// A bare model path (`kit.glb`, no `::`) spawns the whole model: a root named
/// `name` or after the file, one child per node (#1083); its id is the root's.
pub fn instantiate_asset(
    scene: &mut Scene,
    reference: &str,
    name: Option<&str>,
    position: Vec3,
) -> Result<u32, String> {
    if !reference.contains(asset::REF_SEPARATOR) && asset::is_model_path(reference) {
        let asset = asset::import_and_sync_sidecar(std::path::Path::new(reference))
            .map_err(|e| e.to_string())?;
        let root =
            crate::scene::model_instance::instantiate_model(scene, reference, &asset, position);
        if let Some(name) = name {
            scene.world.set_name(root, name.to_string());
        }
        return Ok(root);
    }
    let asset_ref = asset::AssetRef::parse(reference).map_err(|e| e.to_string())?;
    let asset = asset::import_and_sync_sidecar(std::path::Path::new(&asset_ref.path))
        .map_err(|e| e.to_string())?;
    let (path, sub) = (&asset_ref.path, &asset_ref.sub_object);
    instantiate_imported(scene, path, &asset, sub, name, position)
}

/// [`instantiate_asset`] for a file its caller already imported: `asset` is the
/// file at `path`, and `sub` one of its sub-objects (or a `_LOD<n>` set's base).
/// The mesh is built from `asset` itself, so a model spawned whole through here
/// parses its file once, not once more per node.
pub fn instantiate_imported(
    scene: &mut Scene,
    path: &str,
    asset: &Imported,
    sub: &str,
    name: Option<&str>,
    position: Vec3,
) -> Result<u32, String> {
    let reference = asset::AssetRef::new(path, sub)
        .map_err(|e| e.to_string())?
        .to_string_ref();
    let Some(sub_mesh) = asset.sub_mesh(sub) else {
        // A `_LOD<n>` set's base name addresses the whole set (#472).
        return match asset::lod::lod_set(asset, sub) {
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
    // A skinned sub-object brings its skeleton as child bone entities (#453).
    scene.sync_skeleton(id);

    apply_material(scene, id, path, asset, sub);
    if sync_collider(scene, id, path, asset, sub) {
        scene.update_all_colliders();
    }
    Ok(id)
}

/// Resolve the sub-mesh's imported materials into the scene library and reference
/// them from `id`, one per material run (#1285); a run that names no material takes
/// the engine default. When no run names one, dress the entity with the engine
/// default alone — a GameObject built from a glTF carries a Material by definition
/// (CLAUDE.md), so it renders with a resolvable reference rather than none.
fn apply_material(scene: &mut Scene, id: u32, path: &str, asset: &ImportedAsset, sub: &str) {
    let runs: Vec<Option<usize>> = asset
        .sub_mesh(sub)
        .map(|s| s.ranges.iter().map(|r| r.material).collect())
        .unwrap_or_default();
    // Each distinct material is converted once, however many runs draw with it.
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
    // A run with no material, or none at all, draws with the entity's own default.
    if keys.is_empty() || keys.iter().any(Option::is_none) {
        crate::scene::authoring::attach_default_material(scene, id);
    }
    if keys.iter().all(Option::is_none) {
        return;
    }
    let fallback = crate::components::material::entity_material_key(id);
    let materials = keys
        .into_iter()
        .map(|k| k.unwrap_or_else(|| fallback.clone()))
        .collect();
    scene
        .world
        .set_material(id, Some(MaterialComponent { materials }));
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
    // A material already in the scene is the user's (a placed model's recolour,
    // #997): a later spawn of the same file must not reset it.
    scene
        .materials
        .entry(key.clone())
        .or_insert_with(|| material_asset_from_import(data));
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
mod multi_material_tests;
#[cfg(test)]
mod separator_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod texture_tests;

/// Set a model's Generate Lightmap UVs settings (#831) in its `.meta` sidecar and
/// re-import every mesh in `scene` instanced from it, so the change shows at once
/// (Unity's *Apply*). The one verb the model inspector and
/// `Assets.SetLightmapUVSettings` share. Returns how many meshes were re-imported.
pub fn set_lightmap_uv_settings(
    scene: &mut Scene,
    path: &str,
    settings: asset::LightmapUvSettings,
) -> Result<usize, String> {
    asset::sidecar::set_lightmap_uvs(std::path::Path::new(path), settings)
        .map_err(|e| e.to_string())?;
    Ok(rehydrate::reimport_model(scene, path, Rig::Keep))
}

/// Set a model's Scale Factor (#1398) in its `.meta` sidecar and re-import every
/// mesh in `scene` instanced from it, a skinned one with its new rig and its bones
/// moved to the scaled rest pose; an LODGroup over those meshes has its size scaled
/// by as much as its LOD0 meshes grew. A whole-model instance (a linked instance
/// of the model, #1637) is rebuilt from the scaled import, so its parts' offsets
/// scale too and a part moved by hand keeps its override. The model is noted
/// in the write log, so every other cache keyed by it (the particle meshes)
/// re-reads it too. A NaN or unchanged
/// factor changes nothing: no write, no re-import. The one verb the model inspector
/// and `Assets.SetModelImportSettings` share. Returns how many meshes were re-imported.
pub fn set_model_scale_factor(scene: &mut Scene, path: &str, scale: f32) -> Result<usize, String> {
    let source = std::path::Path::new(path);
    let stored = asset::sidecar::load(source)
        .map_err(|e| e.to_string())?
        .scale;
    if asset::scale_factor::sanitize(scale, stored) == stored {
        return Ok(0);
    }
    asset::sidecar::set_scale(source, scale).map_err(|e| e.to_string())?;
    // A whole-model instance is rebuilt from the scaled import, its parts' offsets
    // with it (#1637), so the refit skips its static parts; a lone sub-object only
    // re-imports its mesh.
    let refit = |scene: &mut Scene, skip: &_| {
        let before = crate::scene::lod_instance::lod0_extents(scene);
        let ids = rehydrate::model_writes::reimport_noted_except(scene, path, Rig::Rebuild, skip);
        crate::scene::lod_instance::rescale_groups_over(scene, &ids, &before);
        ids.len() + skip.len()
    };
    // The same match the write-log follow uses, so a link naming the model by
    // another spelling (absolute vs project-relative) is found too.
    let model = crate::core::fs::written::Stale::new(vec![path.to_string()]);
    Ok(crate::scene::prefab::restamp::restamp_models(
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
#[path = "lightmap_tests.rs"]
pub(crate) mod lightmap_test_support;
