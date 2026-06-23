//! src/scene/prefab_apply.rs — apply a linked instance's overrides BACK to source (#268).
//!
//! The missing direction of the prefab override workflow: #216 gave record / revert /
//! reimport (edits that live on the *instance*); this writes an instance's recorded
//! overrides **into the source `.prefab` document on disk**, so the asset itself changes
//! and every *other* instance picks the edit up on its next load / reimport. Two forms:
//! whole-object ([`apply_instance_to_source`]) and a single json-pointer leaf
//! ([`apply_instance_field_to_source`]).
//!
//! It reuses the existing serde-value machinery rather than inventing a parallel
//! serializer: [`apply_overrides`] is the same setter the live propagation uses (now
//! aimed at a raw source entity), [`read_prefab_file`] / [`write_prefab_file`] are the
//! one `.prefab` reader/writer, and clearing the instance's overrides afterwards reuses
//! [`revert_instance_overrides`] / [`reimport_instance`] so the instance re-baselines
//! against the now-updated source. Scope mirrors #216: a flat instance of a flat prefab,
//! matched to source entities purely by the link's `local_id`.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::scene::prefab::{read_prefab_file, write_prefab_file};
use crate::scene::prefab_overrides::apply_overrides;
use crate::scene::{reimport_instance, revert_instance_overrides, PrefabData, Scene};

/// Apply every recorded override on the instance rooted at `root_id` back into its
/// source `.prefab` on disk, then drop those overrides so the instance now matches the
/// source. Returns the number of entities whose overrides were written. Errors if
/// `root_id` is not a linked instance root or the source can't be read/written.
pub fn apply_instance_to_source(scene: &mut Scene, root_id: u32) -> Result<usize, String> {
    let ids = instance_ids(scene, root_id)?;
    let source = link_source(scene, root_id)?;
    let mut prefab = read_prefab_file(&source)?;
    let mut applied = 0;
    for id in ids {
        let overrides = overrides_of(scene, id);
        if overrides.is_empty() {
            continue;
        }
        if let Some(local_id) = local_id_of(scene, id) {
            if write_onto_source(&mut prefab, local_id, &overrides) {
                applied += 1;
            }
        }
    }
    write_prefab_file(&prefab, &source)?;
    // The source now carries the edits; revert rebuilds the instance from it (identical
    // live values) and clears the now-redundant overrides — the instance matches source.
    revert_instance_overrides(scene, root_id)?;
    Ok(applied)
}

/// Apply ONE overridden field back into the source: write the leaf at `path` from
/// `entity_id`'s override map onto its matching source entity, save, then drop just that
/// override and reimport so the leaf re-tracks the source (other overrides untouched).
/// `entity_id` may be any instance entity (root or child). Errors if it is not a linked
/// instance entity, has no override at `path`, or the source can't be read/written.
pub fn apply_instance_field_to_source(
    scene: &mut Scene,
    entity_id: u32,
    path: &str,
) -> Result<(), String> {
    let root = instance_root(scene, entity_id)?;
    let source = link_source(scene, root)?;
    let local_id =
        local_id_of(scene, entity_id).ok_or_else(|| format!("entity {entity_id} is not linked"))?;
    let leaf = overrides_of(scene, entity_id)
        .get(path)
        .cloned()
        .ok_or_else(|| format!("entity {entity_id} has no override at {path}"))?;
    let mut prefab = read_prefab_file(&source)?;
    let one = BTreeMap::from([(path.to_string(), leaf)]);
    if !write_onto_source(&mut prefab, local_id, &one) {
        return Err(format!("source has no entity with local id {local_id}"));
    }
    write_prefab_file(&prefab, &source)?;
    drop_override(scene, entity_id, path);
    reimport_instance(scene, root)
}

/// Write `overrides` onto the source entity with `local_id` inside `prefab`, in place.
/// Reuses [`apply_overrides`] — the same setter the live instance uses — so the
/// write-back and propagation share one mechanism. `false` if no such source entity.
fn write_onto_source(
    prefab: &mut PrefabData,
    local_id: u32,
    overrides: &BTreeMap<String, Value>,
) -> bool {
    match prefab.entities.iter_mut().find(|e| e.id == local_id) {
        Some(entity) => {
            apply_overrides(entity, overrides);
            true
        }
        None => false,
    }
}

/// The entity ids of the instance rooted at `root_id` (root + descendants sharing its
/// prefab source). Errors if `root_id` is not a linked instance root.
fn instance_ids(scene: &Scene, root_id: u32) -> Result<Vec<u32>, String> {
    let source = link_source(scene, root_id)?;
    let mut ids = Vec::new();
    let mut stack = vec![root_id];
    while let Some(id) = stack.pop() {
        let Some(entity) = scene.get_entity(id) else {
            continue;
        };
        if entity.prefab_link.as_ref().is_some_and(|l| l.source == source) {
            ids.push(id);
            stack.extend(entity.children.iter().copied());
        }
    }
    Ok(ids)
}

/// The instance ROOT for any member `id`: walk up while the parent shares `id`'s prefab
/// source. Errors if `id` is not a linked instance entity.
fn instance_root(scene: &Scene, id: u32) -> Result<u32, String> {
    let source = link_source(scene, id)?;
    let mut current = id;
    while let Some(parent) = scene.get_entity(current).and_then(|e| e.parent_id) {
        let shares = scene
            .get_entity(parent)
            .map(|e| e.prefab_link.as_ref().is_some_and(|l| l.source == source))
            .unwrap_or(false);
        if !shares {
            break;
        }
        current = parent;
    }
    Ok(current)
}

/// The source `.prefab` path entity `id` links to, or an error if it isn't linked.
fn link_source(scene: &Scene, id: u32) -> Result<String, String> {
    scene
        .get_entity(id)
        .and_then(|e| e.prefab_link.as_ref().map(|l| l.source.clone()))
        .ok_or_else(|| format!("entity {id} is not a linked prefab instance"))
}

/// The source `local_id` recorded on entity `id`, if it is linked.
fn local_id_of(scene: &Scene, id: u32) -> Option<u32> {
    scene.get_entity(id)?.prefab_link.as_ref().map(|l| l.local_id)
}

/// Entity `id`'s recorded override map (empty if it has none / is unlinked).
fn overrides_of(scene: &Scene, id: u32) -> BTreeMap<String, Value> {
    scene
        .get_entity(id)
        .and_then(|e| e.prefab_link.as_ref().map(|l| l.overrides.clone()))
        .unwrap_or_default()
}

/// Drop one override `path` from entity `id`'s link (a no-op if absent).
fn drop_override(scene: &mut Scene, id: u32, path: &str) {
    if let Some(mut entity) = scene.get_entity_mut(id) {
        if let Some(link) = entity.prefab_link.as_mut() {
            link.overrides.remove(path);
        }
    }
}

#[cfg(test)]
#[path = "prefab_apply_tests.rs"]
mod prefab_apply_tests;
