//! src/scene/prefab_link.rs — linked prefab instances: live link operations (#216).
//!
//! The verbs that act on an already-stamped **linked** instance (one whose entities
//! carry a [`PrefabLink`]): record its per-instance overrides, list them, revert
//! them, and re-import (propagate) the source onto it. They are the apply/revert side
//! of prefabs v2 and the load-time propagation, all sharing one mechanism:
//!
//!   propagate(instance) = rebuild every entity from a FRESH source baseline, then
//!                         re-apply that entity's recorded override map on top.
//!
//! "Fresh source baseline" = the source `.prefab` re-instantiated through the v1
//! material merge/rename + mesh rehydrate, so material refs re-resolve through the
//! same rename map and geometry is rebuilt from the reference (never diffed).
//!
//! Scope (#216): a FLAT instance of a FLAT prefab. Added/removed/reparented child
//! entities and nested prefabs are out of scope — propagation matches an instance
//! entity to a source entity purely by the link's `local_id`, and drops nothing /
//! adds nothing structurally.

use std::collections::BTreeMap;

use crate::components::{Entity, PrefabLink};
use crate::scene::prefab::read_prefab_file;
use crate::scene::prefab_overrides::{apply_overrides, diff_entity};
use crate::scene::serialize::rehydrate_entity_mesh;
use crate::scene::{instantiate_prefab, PrefabData, Scene};

/// The entity ids of the instance rooted at `root_id`: the root plus every
/// descendant that shares the root's `prefab_link.source`. Returns `None` if `root_id`
/// is not a linked instance root.
fn instance_entity_ids(scene: &Scene, root_id: u32) -> Option<Vec<u32>> {
    let source = scene
        .get_entity(root_id)?
        .prefab_link
        .as_ref()?
        .source
        .clone();
    let mut ids = Vec::new();
    let mut stack = vec![root_id];
    while let Some(id) = stack.pop() {
        let Some(entity) = scene.get_entity(id) else {
            continue;
        };
        let same_source = entity
            .prefab_link
            .as_ref()
            .is_some_and(|l| l.source == source);
        if !same_source {
            continue;
        }
        ids.push(id);
        stack.extend(entity.children.iter().copied());
    }
    Some(ids)
}

/// Build the fresh source baselines for one instance, keyed by source `local_id`.
/// Instantiates the source into a SCRATCH scene that already shares `scene`'s
/// material library, so the merge reuses identical entries and rewrites references to
/// the very same library keys the instance uses — then folds any newly-introduced
/// materials back into `scene`. The returned entities are mesh-rehydrated and carry
/// their LOCAL ids (caller overwrites identity/structure from the live instance).
fn fresh_baselines(scene: &mut Scene, prefab: &PrefabData) -> BTreeMap<u32, Entity> {
    let mut scratch = Scene::new();
    scratch.materials = scene.materials.clone();
    let root = instantiate_prefab(&mut scratch, prefab, None);

    // Map each scratch entity back to its source local id via subtree order: the
    // instantiate assigned local `l` -> `base + l`, so subtracting the base recovers
    // `l`. The base is the scratch root's id minus the prefab root's local id.
    let base = root.wrapping_sub(prefab.root);
    let mut out = BTreeMap::new();
    for id in scratch.entity_ids() {
        if let Some(entity) = scratch.get_entity(id) {
            out.insert(id.wrapping_sub(base), (*entity).clone());
        }
    }

    // Carry back any materials the merge added (a brand-new key, or a uniquified one).
    scene.materials = scratch.materials;
    out
}

/// Recompute and store each instance entity's overrides as the diff against its fresh
/// source baseline. This is how an edited instance "captures" its divergence: call it
/// after editing the instance (while the source is unchanged) so propagation later
/// re-applies exactly these paths. Returns the number of entities recorded.
pub fn record_instance_overrides(scene: &mut Scene, root_id: u32) -> Result<usize, String> {
    let ids = require_instance(scene, root_id)?;
    let prefab = load_instance_source(scene, root_id)?;
    let baselines = fresh_baselines(scene, &prefab);
    let mut recorded = 0;
    for id in ids {
        let Some(local_id) = local_id_of(scene, id) else {
            continue;
        };
        let Some(baseline) = baselines.get(&local_id) else {
            continue;
        };
        let overrides = scene
            .get_entity(id)
            .map(|e| diff_entity(&e, baseline))
            .unwrap_or_default();
        if let Some(mut entity) = scene.get_entity_mut(id) {
            if let Some(link) = entity.prefab_link.as_mut() {
                link.overrides = overrides;
                recorded += 1;
            }
        }
    }
    scene.update_all_colliders();
    Ok(recorded)
}

/// Re-import the source onto the instance: rebuild every entity from its fresh
/// baseline, then re-apply its recorded overrides. Non-overridden fields pick up the
/// latest source; overridden ones are preserved. This is the propagation step run on
/// scene load and by the explicit reimport verb.
pub fn reimport_instance(scene: &mut Scene, root_id: u32) -> Result<(), String> {
    let ids = require_instance(scene, root_id)?;
    let prefab = load_instance_source(scene, root_id)?;
    let baselines = fresh_baselines(scene, &prefab);
    for id in ids {
        rebuild_entity(scene, id, &baselines, true);
    }
    scene.update_all_colliders();
    Ok(())
}

/// Drop every override on the instance and rebuild it straight from the fresh source
/// baseline (the instance becomes a pristine copy of the current source).
pub fn revert_instance_overrides(scene: &mut Scene, root_id: u32) -> Result<(), String> {
    let ids = require_instance(scene, root_id)?;
    let prefab = load_instance_source(scene, root_id)?;
    let baselines = fresh_baselines(scene, &prefab);
    for id in ids {
        if let Some(mut entity) = scene.get_entity_mut(id) {
            if let Some(link) = entity.prefab_link.as_mut() {
                link.overrides.clear();
            }
        }
        rebuild_entity(scene, id, &baselines, false);
    }
    scene.update_all_colliders();
    Ok(())
}

/// The recorded override paths of the instance, as `"<local_id>:<json-pointer>"`
/// strings sorted for a stable read. Empty when the instance has no overrides.
pub fn list_instance_overrides(scene: &Scene, root_id: u32) -> Result<Vec<String>, String> {
    let ids = require_instance(scene, root_id)?;
    let mut out = Vec::new();
    for id in ids {
        if let Some(entity) = scene.get_entity(id) {
            if let Some(link) = &entity.prefab_link {
                for path in link.overrides.keys() {
                    out.push(format!("{}:{}", link.local_id, path));
                }
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Re-import EVERY linked instance in the scene (propagation on load). An instance
/// root is a linked entity with `local_id == 0` (the prefab root). A source whose
/// file is missing is skipped (the instance keeps its last-loaded values), so a
/// renamed/deleted `.prefab` never aborts the scene load.
pub fn reimport_all_linked_instances(scene: &mut Scene) {
    let roots: Vec<u32> = scene
        .entity_ids()
        .into_iter()
        .filter(|&id| {
            scene
                .get_entity(id)
                .and_then(|e| e.prefab_link.as_ref().map(|l| l.local_id == 0))
                .unwrap_or(false)
        })
        .collect();
    for root in roots {
        let _ = reimport_instance(scene, root);
    }
}

/// Rebuild one instance entity from its baseline, preserving live identity/structure
/// and (when `keep_overrides`) re-applying its recorded override map.
fn rebuild_entity(scene: &mut Scene, id: u32, baselines: &BTreeMap<u32, Entity>, keep: bool) {
    let Some(local_id) = local_id_of(scene, id) else {
        return;
    };
    let Some(baseline) = baselines.get(&local_id) else {
        return;
    };
    let Some(mut entity) = scene.get_entity_mut(id) else {
        return;
    };
    let (live_id, parent_id, children) = (entity.id, entity.parent_id, entity.children.clone());
    let link = entity.prefab_link.clone();
    let mut rebuilt = baseline.clone();
    rebuilt.id = live_id;
    rebuilt.parent_id = parent_id;
    rebuilt.children = children;
    rebuilt.prefab_link = link;
    if keep {
        if let Some(overrides) = rebuilt.prefab_link.as_ref().map(|l| l.overrides.clone()) {
            apply_overrides(&mut rebuilt, &overrides);
        }
    }
    rehydrate_entity_mesh(&mut rebuilt);
    *entity = rebuilt;
}

/// The instance ids, erroring if `root_id` is not a linked instance root.
fn require_instance(scene: &Scene, root_id: u32) -> Result<Vec<u32>, String> {
    instance_entity_ids(scene, root_id)
        .ok_or_else(|| format!("entity {root_id} is not a linked prefab instance"))
}

/// The source `local_id` recorded on entity `id`, if it is linked.
fn local_id_of(scene: &Scene, id: u32) -> Option<u32> {
    scene
        .get_entity(id)?
        .prefab_link
        .as_ref()
        .map(|l| l.local_id)
}

/// Read the `.prefab` file an instance's root links to.
fn load_instance_source(scene: &Scene, root_id: u32) -> Result<PrefabData, String> {
    let source = link_source(scene, root_id)?;
    read_prefab_file(&source)
}

/// The source path recorded on the instance root.
fn link_source(scene: &Scene, root_id: u32) -> Result<String, String> {
    scene
        .get_entity(root_id)
        .and_then(|e| {
            e.prefab_link
                .as_ref()
                .map(|l: &PrefabLink| l.source.clone())
        })
        .ok_or_else(|| format!("entity {root_id} is not a linked prefab instance"))
}

#[cfg(test)]
#[path = "prefab_link_tests.rs"]
mod prefab_link_tests;
