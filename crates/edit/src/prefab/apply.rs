//! A model source is read-only: applying to it errors, as Unity cannot apply to a model prefab;
//! reverting a field works as on any instance.

use std::collections::BTreeMap;

use crate::prefab::instance::{self, instance_members};
use crate::prefab::link::{propagate, propagate_from};
use crate::prefab::live::sync_overrides;
use crate::prefab::overrides::apply_overrides;
use crate::prefab::source::{gone_node, read_source, read_writable};
use crate::prefab::write_prefab_file;
use crate::{revert_instance_overrides, PrefabData};
use rusty_components::{visit_override_refs, PrefabOverrides};
use rusty_scene::Scene;

pub fn apply_instance_to_source(scene: &mut Scene, root_id: u32) -> Result<usize, String> {
    let ids = instance_ids(scene, root_id)?;
    let source = link_source(scene, root_id)?;
    let mut prefab = read_writable(&source)?;
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
    // The source now holds the edits: rebuilding from it drops the redundant overrides.
    revert_instance_overrides(scene, root_id)?;
    Ok(applied)
}

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
    let mut prefab = read_writable(&source)?;
    let one = to_local_refs(scene, root, &one);
    if !write_onto_source(&mut prefab, local_id, &one) {
        return Err(format!("source has no entity with local id {local_id}"));
    }
    write_prefab_file(&prefab, &source)?;
    drop_override(scene, entity_id, path);
    propagate(scene, root)
}

/// `Ok(false)` when there is no override at `path`. Errors leave everything unchanged: the source
/// is read before the override goes. Unity's `PrefabUtility.RevertPropertyOverride`.
pub fn revert_instance_field(
    scene: &mut Scene,
    entity_id: u32,
    path: &str,
) -> Result<bool, String> {
    let root = instance_root(scene, entity_id)?;
    let source = link_source(scene, root)?;
    let prefab = read_source(&source)?;
    if let Some(node) = gone_node(&scene.world, entity_id, &prefab) {
        return Err(format!(
            "{node} is gone from {source}: there is nothing to revert it to (a reimport removes it)"
        ));
    }
    let ids = instance_ids(scene, root)?;
    sync_overrides(&mut scene.world, &ids);
    if !drop_override(scene, entity_id, path) {
        return Ok(false);
    }
    propagate_from(scene, root, &source, &prefab)?;
    Ok(true)
}

struct LocalOverrides {
    values: PrefabOverrides,
    bones: BTreeMap<String, Option<Vec<String>>>,
}

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

/// A reference outside the instance is dropped, as prefab extraction drops it. One naming a bone of
/// the instance names its skinned entity, the bone kept by name.
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

fn instance_ids(scene: &Scene, root_id: u32) -> Result<Vec<u32>, String> {
    instance_members(scene, root_id).ok_or_else(|| not_linked(root_id))
}

fn instance_root(scene: &Scene, id: u32) -> Result<u32, String> {
    instance::instance_root(scene, id).ok_or_else(|| not_linked(id))
}

fn not_linked(id: u32) -> String {
    format!("entity {id} is not a linked prefab instance")
}

fn link_source(scene: &Scene, id: u32) -> Result<String, String> {
    scene
        .world
        .prefab_link(id)
        .map(|l| l.source.clone())
        .ok_or_else(|| not_linked(id))
}

fn local_id_of(scene: &Scene, id: u32) -> Option<u32> {
    scene.world.prefab_link(id).map(|l| l.local_id)
}

fn overrides_of(scene: &Scene, id: u32) -> PrefabOverrides {
    scene
        .world
        .prefab_link(id)
        .map(|l| l.overrides.clone())
        .unwrap_or_default()
}

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
