//! src/scene/prefab_link.rs — linked prefab instances: live link operations (#216).
//!
//! The verbs that act on an already-stamped **linked** instance (one whose entities
//! carry a [`PrefabLink`](crate::components::PrefabLink)): list its per-instance
//! overrides, revert them, and re-import (propagate) the source onto it. They are the apply/revert side
//! of prefabs v2 and the load-time propagation, all sharing one mechanism:
//!
//!   propagate(instance) = rebuild every entity from a FRESH source baseline, then
//!                         re-apply that entity's override map on top.
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
use std::sync::Arc;

use crate::components::{Entity, EntityRefs};
use crate::scene::authoring::material::drop_unused;
use crate::scene::prefab::diff::masked;
use crate::scene::prefab::instance::instance_members;
use crate::scene::prefab::live::{entity_overrides, keep_root_placement, sync_overrides};
use crate::scene::prefab::overrides::apply_overrides;
use crate::scene::prefab::read_prefab_file;
use crate::scene::serialize::rehydrate_entity_mesh;
use crate::scene::{instantiate_prefab, PrefabData, Scene};

/// Build the fresh source baselines for one instance, keyed by source `local_id`.
/// Instantiates the source into a SCRATCH scene that already shares `scene`'s
/// material library, so the merge reuses identical entries and rewrites references to
/// the very same library keys the instance uses — then folds any newly-introduced
/// materials back into `scene`. The returned entities are mesh-rehydrated and carry
/// their LOCAL ids, component references included (callers move those into the
/// instance's ids with [`in_instance_space`] and overwrite identity/structure). A
/// reference to one of the scratch skeleton's bones keeps the bone by name in
/// `bone_refs` (#1264), naming its skinned entity meanwhile. Also returns the keys
/// the merge added, for [`drop_unused`] once the instance is rebuilt (#1576).
fn fresh_baselines(scene: &mut Scene, prefab: &PrefabData) -> (BTreeMap<u32, Entity>, Vec<String>) {
    let mut scratch = Scene::new();
    scratch.materials = scene.materials.clone();
    let root = instantiate_prefab(&mut scratch, prefab, None);

    // Map each scratch entity back to its source local id via subtree order: the
    // instantiate assigned local `l` -> `base + l`, so subtracting the base recovers
    // `l`. The base is the scratch root's id minus the prefab root's local id.
    let base = root.wrapping_sub(prefab.root);
    let mut docs: Vec<Entity> = scratch
        .entity_ids()
        .into_iter()
        .filter_map(|id| scratch.world.entity_document(id))
        .collect();
    scratch.bone_doc_refs(&mut docs);
    let mut out = BTreeMap::new();
    for mut entity in docs {
        entity.remap_refs(&|r| Some(r.wrapping_sub(base)));
        out.insert(entity.id.wrapping_sub(base), entity);
    }

    // Carry back any materials the merge added (a brand-new key, or a uniquified one).
    let added = scratch
        .materials
        .keys()
        .filter(|k| !scene.materials.contains_key(*k));
    let added = added.cloned().collect();
    scene.materials = scratch.materials;
    (out, added)
}

/// `baselines` (keyed by local id) with their component references moved from
/// local ids to the ids of the instance entities `ids`, so an untouched reference
/// diffs clean and a rebuilt one points inside its own instance; one kept by bone
/// name is bound to the instance's live bone.
fn in_instance_space(
    scene: &Scene,
    ids: &[u32],
    mut baselines: BTreeMap<u32, Entity>,
) -> BTreeMap<u32, Entity> {
    let map: BTreeMap<u32, u32> = ids
        .iter()
        .filter_map(|&id| Some((local_id_of(scene, id)?, id)))
        .collect();
    for (local, entity) in baselines.iter_mut() {
        entity.remap_refs(&|l| map.get(&l).copied());
        let holder = map.get(local).copied().unwrap_or(entity.id);
        scene.bind_doc_bone_refs(entity, holder);
    }
    baselines
}

/// Re-import the source onto the instance: rebuild every entity from its fresh
/// baseline, then re-apply its overrides, the edits made since it last took the
/// source among them (`live/`, #1527). Non-overridden fields pick up the latest
/// source; overridden ones are preserved. This is the propagation step run on scene
/// load and by the explicit reimport verb.
pub fn reimport_instance(scene: &mut Scene, root_id: u32) -> Result<(), String> {
    let ids = require_instance(scene, root_id)?;
    sync_overrides(&mut scene.world, &ids);
    propagate(scene, root_id)
}

/// [`reimport_instance`] re-applying the overrides the links hold as they are, for a
/// caller that has just synced them and dropped one (a per-field revert or apply).
pub(super) fn propagate(scene: &mut Scene, root_id: u32) -> Result<(), String> {
    let ids = require_instance(scene, root_id)?;
    let prefab = load_instance_source(scene, root_id)?;
    let (baselines, added) = fresh_baselines(scene, &prefab);
    let baselines = in_instance_space(scene, &ids, baselines);
    for id in ids {
        rebuild_entity(scene, id, &baselines, true);
    }
    // Only what a slot still names after the overrides stays: a material deleted
    // from the scene is not brought back by its prefab's copy (#1576).
    drop_unused(scene, &added);
    // A rebuilt skinned mesh lost its bone binding; re-bind it by name (#453).
    scene.sync_skeletons();
    scene.update_all_colliders();
    Ok(())
}

/// Drop every override on the instance and rebuild it straight from the fresh source
/// baseline (the instance becomes a pristine copy of the current source).
pub fn revert_instance_overrides(scene: &mut Scene, root_id: u32) -> Result<(), String> {
    let ids = require_instance(scene, root_id)?;
    let prefab = load_instance_source(scene, root_id)?;
    let (baselines, added) = fresh_baselines(scene, &prefab);
    let baselines = in_instance_space(scene, &ids, baselines);
    for id in ids {
        if let Some(mut link) = scene.world.prefab_link_mut(id) {
            link.overrides = Default::default();
        }
        rebuild_entity(scene, id, &baselines, false);
    }
    drop_unused(scene, &added);
    scene.sync_skeletons();
    scene.update_all_colliders();
    Ok(())
}

/// The live override paths of the instance, as `"<local_id>:<json-pointer>"`
/// strings sorted for a stable read. Empty when the instance has no overrides.
pub fn list_instance_overrides(scene: &Scene, root_id: u32) -> Result<Vec<String>, String> {
    let ids = require_instance(scene, root_id)?;
    let mut out = Vec::new();
    for id in ids {
        let Some(local_id) = local_id_of(scene, id) else {
            continue;
        };
        for key in entity_overrides(&scene.world, id).keys() {
            out.push(format!("{local_id}:{key}"));
        }
    }
    out.sort();
    Ok(out)
}

/// Propagate EVERY linked instance in the scene, re-applying the overrides each link
/// holds: a loaded file's, or a Play snapshot's, written while its bones still had
/// their ids (#1527). So a source changed meanwhile (on disk, or applied to during
/// Play) reaches each instance, and every baseline is rebuilt against the live
/// bones, which a load or a Stop respawns with fresh ids. An instance root is a
/// linked entity with `local_id == 0` (the prefab root). A source whose file is
/// missing is skipped, so a renamed/deleted `.prefab` never aborts the scene load:
/// the instance keeps its values and the overrides its links hold, and has no
/// baseline (the document it came from carries none), so an edit made meanwhile is
/// not tracked until the source is back.
pub fn propagate_all_linked_instances(scene: &mut Scene) {
    let roots: Vec<u32> = scene
        .world
        .ids_with_prefab_link()
        .into_iter()
        .filter(|&id| scene.world.prefab_link(id).is_some_and(|l| l.local_id == 0))
        .collect();
    for root in roots {
        let _ = propagate(scene, root);
    }
}

/// Rebuild one instance entity from its baseline, preserving live identity/structure
/// and (when `keep`) re-applying its override map; the baseline it was built from is
/// stored on its link, and a root keeps its own placement.
fn rebuild_entity(scene: &mut Scene, id: u32, baselines: &BTreeMap<u32, Entity>, keep: bool) {
    let Some(local_id) = local_id_of(scene, id) else {
        return;
    };
    let Some(baseline) = baselines.get(&local_id) else {
        return;
    };
    if !scene.world.contains(id) {
        return;
    }
    let parent_id = scene.world.parent_id(id);
    let children = scene.world.children(id);
    let placement = scene.world.transform(id).map(|t| (*t).clone());
    let mut rebuilt = baseline.clone();
    rebuilt.id = id;
    rebuilt.parent_id = parent_id;
    rebuilt.children = children;
    rebuilt.prefab_link = scene.world.prefab_link(id).map(|l| (*l).clone());
    if let Some(link) = rebuilt.prefab_link.as_mut() {
        link.baseline = Some(Arc::new(masked(baseline)));
    }
    if keep {
        if let Some(overrides) = rebuilt.prefab_link.as_ref().map(|l| l.overrides.clone()) {
            apply_overrides(&mut rebuilt, &overrides);
        }
    }
    keep_root_placement(&mut rebuilt, placement.as_ref());
    rehydrate_entity_mesh(&mut rebuilt);
    scene.world.replace_entity(rebuilt);
    // The source file and the overrides are held as a scene file is (#1418).
    crate::scene::authoring::fields::hold_entity(&mut scene.world, id);
}

/// The instance ids, erroring if `root_id` is not a linked instance root.
fn require_instance(scene: &Scene, root_id: u32) -> Result<Vec<u32>, String> {
    instance_members(scene, root_id)
        .ok_or_else(|| format!("entity {root_id} is not a linked prefab instance"))
}

/// The source `local_id` recorded on entity `id`, if it is linked.
fn local_id_of(scene: &Scene, id: u32) -> Option<u32> {
    scene.world.prefab_link(id).map(|l| l.local_id)
}

/// Read the `.prefab` file an instance's root links to.
fn load_instance_source(scene: &Scene, root_id: u32) -> Result<PrefabData, String> {
    let source = link_source(scene, root_id)?;
    read_prefab_file(&source)
}

/// The source path recorded on the instance root.
fn link_source(scene: &Scene, root_id: u32) -> Result<String, String> {
    scene
        .world
        .prefab_link(root_id)
        .map(|l| l.source.clone())
        .ok_or_else(|| format!("entity {root_id} is not a linked prefab instance"))
}

#[cfg(test)]
#[path = "link_tests.rs"]
mod prefab_link_tests;
#[cfg(test)]
#[path = "lod_override_tests.rs"]
mod prefab_lod_override_tests;
#[cfg(test)]
#[path = "refs_tests.rs"]
mod prefab_refs_tests;
