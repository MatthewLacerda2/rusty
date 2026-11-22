//! propagate(instance) = rebuild every entity from a FRESH source baseline, then re-apply that entity's override map on top.
//! A `.prefab` instance is flat: entities match by the link's `local_id` only. Nested prefabs are out of scope: an instance's walk stops at another instance's root.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::prefab::diff::masked;
use crate::prefab::instance::instance_members;
use crate::prefab::live::{entity_overrides, keep_root_placement, sync_overrides};
use crate::prefab::overrides::apply_overrides;
use crate::prefab::pass::Pass;
use crate::prefab::restamp::{instance_roots, propagate_roots};
use crate::prefab::source::{is_model_source, read_source, rekey, GONE};
use crate::prefab::structure::reshape;
use crate::PrefabData;
use rusty_components::{Entity, EntityRefs};
use rusty_scene::rehydrate::{adopt_or_rehydrate, Imports, Keep};
use rusty_scene::Scene;

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

pub fn reimport_instance(scene: &mut Scene, root_id: u32) -> Result<(), String> {
    let ids = require_instance(scene, root_id)?;
    sync_overrides(&mut scene.world, &ids);
    propagate(scene, root_id)
}

pub(super) fn propagate(scene: &mut Scene, root_id: u32) -> Result<(), String> {
    let source = link_source(scene, root_id)?;
    propagate_from(scene, root_id, &source, &read_source(&source)?)
}

pub(super) fn propagate_from(
    scene: &mut Scene,
    root_id: u32,
    source: &str,
    prefab: &PrefabData,
) -> Result<(), String> {
    let mut imports = Imports::default();
    let mut pass = Pass::new(&mut imports, false);
    rebuild_instance(scene, root_id, source, prefab, true, &mut pass)?;
    pass.finish(scene);
    finish_rebuild(scene);
    Ok(())
}

pub(super) fn rebuild_instance(
    scene: &mut Scene,
    root_id: u32,
    source: &str,
    prefab: &PrefabData,
    keep: bool,
    pass: &mut Pass,
) -> Result<(), String> {
    let mut ids = require_instance(scene, root_id)?;
    if !keep {
        for &id in &ids {
            if let Some(mut link) = scene.world.prefab_link_mut(id) {
                link.overrides = Default::default();
            }
        }
    }
    rekey(&mut scene.world, &ids, prefab);
    if is_model_source(source) {
        ids = reshape(scene, root_id, &ids, source, prefab, pass);
    }
    let baselines = pass.baselines(scene, source, prefab);
    let baselines = in_instance_space(scene, &ids, baselines);
    for id in ids {
        rebuild_entity(scene, id, &baselines, keep, pass.imports);
    }
    Ok(())
}

pub(super) fn finish_rebuild(scene: &mut Scene) {
    scene.sync_skeletons();
    scene.sync_scene_meshes();
    scene.update_all_colliders();
}

pub fn revert_instance_overrides(scene: &mut Scene, root_id: u32) -> Result<(), String> {
    require_instance(scene, root_id)?;
    let source = link_source(scene, root_id)?;
    let prefab = read_source(&source)?;
    let mut imports = Imports::default();
    let mut pass = Pass::new(&mut imports, false);
    rebuild_instance(scene, root_id, &source, &prefab, false, &mut pass)?;
    pass.finish(scene);
    finish_rebuild(scene);
    Ok(())
}

pub fn list_instance_overrides(scene: &Scene, root_id: u32) -> Result<Vec<String>, String> {
    let ids = require_instance(scene, root_id)?;
    let mut out = Vec::new();
    for id in ids {
        // A part whose node the model lost waits for Play's destroy phase, and has no local id to list it under.
        let Some(link) = scene.world.prefab_link(id).filter(|l| l.local_id != GONE) else {
            continue;
        };
        for key in entity_overrides(&scene.world, id).keys() {
            out.push(format!("{}:{key}", link.local_id));
        }
    }
    out.sort();
    Ok(out)
}

/// A source whose file is missing is skipped, so a deleted `.prefab` never aborts a scene load: the instance keeps its values and overrides, with no baseline until the source is back.
pub fn propagate_linked_instances_in(scene: &mut Scene, imports: &mut Imports) {
    let roots = instance_roots(scene, |_| true);
    propagate_roots(scene, roots, true, imports);
}

fn rebuild_entity(
    scene: &mut Scene,
    id: u32,
    baselines: &BTreeMap<u32, Entity>,
    keep: bool,
    imports: &mut Imports,
) {
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
    adopt_or_rehydrate(&mut scene.world, &mut rebuilt, Keep::Current, imports);
    scene.world.replace_entity(rebuilt);
    crate::authoring::fields::hold_entity(&mut scene.world, id);
}

fn require_instance(scene: &Scene, root_id: u32) -> Result<Vec<u32>, String> {
    instance_members(scene, root_id)
        .ok_or_else(|| format!("entity {root_id} is not a linked prefab instance"))
}

fn local_id_of(scene: &Scene, id: u32) -> Option<u32> {
    scene.world.prefab_link(id).map(|l| l.local_id)
}

pub(super) fn link_source(scene: &Scene, root_id: u32) -> Result<String, String> {
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
