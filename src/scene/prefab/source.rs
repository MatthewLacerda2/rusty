//! src/scene/prefab/source.rs — what a linked instance's source is (#1637).
//!
//! A source is a `.prefab` document, or an imported model: Unity's *model
//! prefab*. A model's [`PrefabData`] is not stored anywhere: it is built from the
//! current import (`asset::imported`, so a Scale Factor or a re-written file is
//! what it reads) by spawning the node tree into a scratch scene and extracting it
//! (`scene::model_instance::model_prefab`). The rest of the prefab machinery
//! (stamp, propagation, overrides) does not know the difference.
//!
//! A model's local ids follow its node order, which a re-import may change (a node
//! added in Blender shifts every node after it), so each entity of a model
//! instance also keeps its **node path** ([`node_paths`]) on its link, and
//! propagation re-derives the local ids from it first ([`rekey`]). A model is
//! read-only: an instance's overrides are never applied back to it
//! ([`read_writable`]), as Unity cannot apply to a model prefab.

use std::collections::BTreeMap;

use crate::ecs::World;
use crate::scene::prefab::{is_prefab_path, read_prefab_file, PrefabData};

/// The local id an entity of a model instance takes when its node is gone from the
/// model's current import: no source entity has it, so propagation leaves the
/// entity as it is (structural propagation is #1665).
pub const GONE: u32 = u32::MAX;

/// Whether `source` (a link's source path) is a model rather than a `.prefab`.
pub fn is_model_source(source: &str) -> bool {
    !is_prefab_path(source) && crate::asset::is_model_path(source)
}

/// The source at `path` as a prefab: the `.prefab` document, or the model's node
/// tree as its current import builds it.
pub fn read_source(path: &str) -> Result<PrefabData, String> {
    if !is_model_source(path) {
        return read_prefab_file(path);
    }
    let asset = crate::asset::imported(std::path::Path::new(path)).map_err(|e| e.to_string())?;
    Ok(crate::scene::model_instance::model_prefab(path, &asset))
}

/// [`read_source`] for a verb that writes the source back: a model refuses.
pub fn read_writable(path: &str) -> Result<PrefabData, String> {
    if is_model_source(path) {
        return Err(format!(
            "{path} is a model: an instance's overrides can't be applied to it (save it as a prefab first)"
        ));
    }
    read_prefab_file(path)
}

/// The node path of every entity of `prefab`, by local id: the root is `""`, a
/// child its parent's path and its own name joined by `/` (a node a bone holds is
/// `<bone>><name>`, by the nearest bone: `parent_bone` is nearest first), and the second sibling sharing a name takes `[1]`, the third
/// `[2]`. Parents come before their children in `prefab.entities`, as
/// `extract_prefab` writes them; an entity whose parent is not yet known is
/// skipped.
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

/// Point every member of a model instance (`ids`) at the local id its node path
/// has in `prefab`, the model as it imports now; a node the model lost takes
/// [`GONE`]. An entity with no node path (a `.prefab` instance's) is left alone.
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

/// The node path of `id` when it is a model part whose node `prefab` (the model
/// as it imports now) no longer has: there is nothing to rebuild or revert it to.
pub fn gone_node(world: &World, id: u32, prefab: &PrefabData) -> Option<String> {
    let node = world.prefab_link(id)?.node.clone()?;
    let gone = !node_paths(prefab).values().any(|p| *p == node);
    gone.then_some(node)
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod source_tests;
