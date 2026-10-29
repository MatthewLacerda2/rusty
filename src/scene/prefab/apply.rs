//! src/scene/prefab_apply.rs — apply a linked instance's overrides BACK to source (#268).
//!
//! The missing direction of the prefab override workflow: #216 gave revert / reimport
//! (edits that live on the *instance*); this writes an instance's live
//! overrides **into the source `.prefab` document on disk**, so the asset itself changes
//! and every *other* instance picks the edit up on its next load / reimport. Two forms:
//! whole-object ([`apply_instance_to_source`]) and a single json-pointer leaf
//! ([`apply_instance_field_to_source`]). Its per-field opposite, dropping one override
//! so the leaf tracks the source again, lives here too ([`revert_instance_field`]).
//!
//! It reuses the existing serde-value machinery rather than inventing a parallel
//! serializer: [`apply_overrides`] is the same setter the live propagation uses (now
//! aimed at a raw source entity), [`read_prefab_file`] / [`write_prefab_file`] are the
//! one `.prefab` reader/writer, and clearing the instance's overrides afterwards reuses
//! [`revert_instance_overrides`] / `link::propagate` so the instance re-baselines
//! against the now-updated source. Scope mirrors #216: a flat instance of a flat prefab,
//! matched to source entities purely by the link's `local_id`.

use std::collections::BTreeMap;

use crate::components::{visit_override_refs, PrefabOverrides};
use crate::scene::prefab::instance::{self, instance_members};
use crate::scene::prefab::link::propagate;
use crate::scene::prefab::live::sync_overrides;
use crate::scene::prefab::overrides::apply_overrides;
use crate::scene::prefab::{read_prefab_file, write_prefab_file};
use crate::scene::{revert_instance_overrides, PrefabData, Scene};

/// Apply every override on the instance rooted at `root_id` back into its
/// source `.prefab` on disk, then drop those overrides so the instance now matches the
/// source. Returns the number of entities whose overrides were written. Errors if
/// `root_id` is not a linked instance root or the source can't be read/written.
pub fn apply_instance_to_source(scene: &mut Scene, root_id: u32) -> Result<usize, String> {
    let ids = instance_ids(scene, root_id)?;
    let source = link_source(scene, root_id)?;
    let mut prefab = read_prefab_file(&source)?;
    sync_overrides(&mut scene.world, &ids);
    let mut applied = 0;
    for id in ids {
        let overrides = overrides_of(scene, id);
        if overrides.is_empty() {
            continue;
        }
        if let Some(local_id) = local_id_of(scene, id) {
            let local = to_local_refs(scene, root_id, &overrides);
            if write_onto_source(&mut prefab, local_id, &local) {
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

/// Apply ONE override back into the source: write the override at `path` (a key of
/// `Scene.ListPrefabOverrides`: a leaf's pointer, a list's `/#size`, `+/<component>`
/// or `-/<component>`) from `entity_id`'s overrides onto its matching source entity,
/// save, then drop just that override and reimport so it re-tracks the source.
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
    let ids = instance_ids(scene, root)?;
    sync_overrides(&mut scene.world, &ids);
    let one = overrides_of(scene, entity_id)
        .only(path)
        .ok_or_else(|| format!("entity {entity_id} has no override at {path}"))?;
    let mut prefab = read_prefab_file(&source)?;
    let one = to_local_refs(scene, root, &one);
    if !write_onto_source(&mut prefab, local_id, &one) {
        return Err(format!("source has no entity with local id {local_id}"));
    }
    write_prefab_file(&prefab, &source)?;
    drop_override(scene, entity_id, path);
    propagate(scene, root)
}

/// Revert ONE overridden field: drop the override at `path` from `entity_id`'s map,
/// then reimport the instance so that leaf tracks the source again while every other
/// override stays (Unity's `PrefabUtility.RevertPropertyOverride`). `entity_id` may be
/// any instance entity (root or child). `Ok(false)` when there is no override at
/// `path`; errors if `entity_id` is not a linked instance entity or the source can't
/// be read. Either way nothing changes: the source is read before the override goes.
/// The editor's per-field revert button and `Scene.RevertPrefabField` (#998).
pub fn revert_instance_field(
    scene: &mut Scene,
    entity_id: u32,
    path: &str,
) -> Result<bool, String> {
    let root = instance_root(scene, entity_id)?;
    read_prefab_file(&link_source(scene, root)?)?;
    let ids = instance_ids(scene, root)?;
    sync_overrides(&mut scene.world, &ids);
    if !drop_override(scene, entity_id, path) {
        return Ok(false);
    }
    propagate(scene, root)?;
    Ok(true)
}

/// Overrides in the source's local ids, and for each id leaf the bone it names by
/// name (`Some`), or none, so a stale `bone_refs` entry goes (`None`).
struct LocalOverrides {
    values: PrefabOverrides,
    bones: BTreeMap<String, Option<Vec<String>>>,
}

/// Write `overrides` onto the source entity with `local_id` inside `prefab`, in place.
/// Reuses [`apply_overrides`] — the same setter the live instance uses — so the
/// write-back and propagation share one mechanism. `false` if no such source entity.
fn write_onto_source(prefab: &mut PrefabData, local_id: u32, overrides: &LocalOverrides) -> bool {
    let Some(entity) = prefab.entities.iter_mut().find(|e| e.id == local_id) else {
        return false;
    };
    apply_overrides(entity, &overrides.values);
    for (path, chain) in &overrides.bones {
        match chain {
            Some(chain) => entity.bone_refs.insert(path.clone(), chain.clone()),
            None => entity.bone_refs.remove(path),
        };
    }
    true
}

/// `overrides` with every entity reference (a Selectable's target, #420, a joint
/// inside an added component) moved from the instance's ids to the source's local
/// ids — a reference outside the instance is dropped, as prefab extraction drops it.
/// One naming a bone of the instance names its skinned entity, the bone kept by name
/// (#1264).
fn to_local_refs(scene: &Scene, root_id: u32, overrides: &PrefabOverrides) -> LocalOverrides {
    let local: BTreeMap<u32, u32> = instance_ids(scene, root_id)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|id| Some((id, local_id_of(scene, id)?)))
        .collect();
    let mut values = overrides.clone();
    let mut bones = BTreeMap::new();
    visit_override_refs(&mut values, &mut |path, r| {
        let (target, chain) = match r.and_then(|id| scene.bone_ref(id)) {
            Some((owner, chain)) => (Some(owner), Some(chain)),
            None => (*r, None),
        };
        *r = target.and_then(|id| local.get(&id).copied());
        bones.insert(path.to_string(), chain.filter(|_| r.is_some()));
    });
    LocalOverrides { values, bones }
}

/// The instance rooted at `root_id` ([`instance_members`]). Errors if `root_id` is not
/// a linked instance root.
fn instance_ids(scene: &Scene, root_id: u32) -> Result<Vec<u32>, String> {
    instance_members(scene, root_id).ok_or_else(|| not_linked(root_id))
}

/// The instance root for any member `id` ([`instance::instance_root`]). Errors if
/// `id` is not a linked instance entity.
fn instance_root(scene: &Scene, id: u32) -> Result<u32, String> {
    instance::instance_root(scene, id).ok_or_else(|| not_linked(id))
}

fn not_linked(id: u32) -> String {
    format!("entity {id} is not a linked prefab instance")
}

/// The source `.prefab` path entity `id` links to, or an error if it isn't linked.
fn link_source(scene: &Scene, id: u32) -> Result<String, String> {
    scene
        .world
        .prefab_link(id)
        .map(|l| l.source.clone())
        .ok_or_else(|| not_linked(id))
}

/// The source `local_id` recorded on entity `id`, if it is linked.
fn local_id_of(scene: &Scene, id: u32) -> Option<u32> {
    scene.world.prefab_link(id).map(|l| l.local_id)
}

/// Entity `id`'s overrides as last synced (empty if it has none / is unlinked).
fn overrides_of(scene: &Scene, id: u32) -> PrefabOverrides {
    scene
        .world
        .prefab_link(id)
        .map(|l| l.overrides.clone())
        .unwrap_or_default()
}

/// Drop override `key` from entity `id`'s link; whether there was one to drop.
fn drop_override(scene: &mut Scene, id: u32, key: &str) -> bool {
    scene
        .world
        .prefab_link_mut(id)
        .is_some_and(|mut link| link.overrides.remove(key))
}

#[cfg(test)]
#[path = "apply_tests.rs"]
mod prefab_apply_tests;

#[cfg(test)]
#[path = "revert_field_tests.rs"]
mod prefab_revert_field_tests;
