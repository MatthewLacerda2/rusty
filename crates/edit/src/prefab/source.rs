//! A model's local ids follow its node order, which a re-import may change (a node added in Blender
//! shifts every later one), so each entity keeps its node path on its link and propagation
//! re-derives the local ids from it. A model is read-only, as Unity's model prefab.

use std::collections::BTreeMap;

use crate::prefab::{read_prefab_file, PrefabData};
use rusty_ecs::World;

pub use rusty_scene::prefab_link::{is_model_source, GONE};

pub fn read_source(path: &str) -> Result<PrefabData, String> {
    read_source_in(path, &mut rusty_scene::rehydrate::Imports::default())
}

pub fn read_source_in(
    path: &str,
    imports: &mut rusty_scene::rehydrate::Imports,
) -> Result<PrefabData, String> {
    if !is_model_source(path) {
        return read_prefab_file(path);
    }
    let asset = imports.of(path).map_err(|e| e.to_string())?;
    Ok(crate::model_instance::model_prefab(path, &asset))
}

pub fn read_writable(path: &str) -> Result<PrefabData, String> {
    if is_model_source(path) {
        return Err(format!(
            "{path} is a model: an instance's overrides can't be applied to it (save it as a prefab first)"
        ));
    }
    read_prefab_file(path)
}

/// A node a bone holds is `<bone>><name>`; the second sibling sharing a name takes `[1]`, the third
/// `[2]`.
pub fn node_paths(prefab: &PrefabData) -> BTreeMap<u32, String> {
    let mut paths: BTreeMap<u32, String> = BTreeMap::new();
    let mut seen: BTreeMap<String, u32> = BTreeMap::new();
    for entity in &prefab.entities {
        let Some(parent) = entity.parent_id else {
            paths.insert(entity.id, String::new());
            continue;
        };
        let Some(parent_path) = paths.get(&parent) else {
            continue;
        };
        let segment = match entity.parent_bone.first() {
            Some(bone) => format!("{bone}>{}", entity.name),
            None => entity.name.clone(),
        };
        let path = match parent_path.is_empty() {
            true => segment,
            false => format!("{parent_path}/{segment}"),
        };
        let count = seen.entry(path.clone()).or_insert(0);
        let path = match *count {
            0 => path,
            n => format!("{path}[{n}]"),
        };
        *count += 1;
        paths.insert(entity.id, path);
    }
    paths
}

pub fn rekey(world: &mut World, ids: &[u32], prefab: &PrefabData) {
    let by_path: BTreeMap<String, u32> = node_paths(prefab)
        .into_iter()
        .map(|(local, path)| (path, local))
        .collect();
    for &id in ids {
        let Some(node) = world.prefab_link(id).and_then(|l| l.node.clone()) else {
            continue;
        };
        let local = by_path.get(&node).copied().unwrap_or(GONE);
        if let Some(mut link) = world.prefab_link_mut(id) {
            link.local_id = local;
        }
    }
}

pub fn gone_node(world: &World, id: u32, prefab: &PrefabData) -> Option<String> {
    let node = world.prefab_link(id)?.node.clone()?;
    let gone = !node_paths(prefab).values().any(|p| *p == node);
    gone.then_some(node)
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod source_tests;
