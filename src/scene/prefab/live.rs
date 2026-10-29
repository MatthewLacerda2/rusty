//! src/scene/prefab/live.rs — an instance edit is an override the moment it is made (#1527).
//!
//! Every entity of a linked instance keeps the source as it last took it
//! ([`PrefabLink::baseline`](crate::components::PrefabLink)): its leaves as the stamp or
//! the last propagation built them, before any override. Its live overrides are its
//! diff against that baseline, so an edit is an override whatever made it (the gizmo,
//! the inspector, a Lua setter, the MCP), with no hook in any setter, as in Unity.
//!
//! The diff is taken against the source as last *propagated*, never the file as it
//! is now: a source changed since would turn every leaf the instance never touched
//! into an override, and the instance would stop tracking the source. So the live
//! set is stored on the link ([`sync_overrides`]) before a propagation re-reads the
//! source, and a save or a Play snapshot writes it ([`write_overrides`]).
//!
//! A stored override the source can no longer hold (its path is in neither the
//! entity nor its baseline) is *unused*, and kept, as Unity keeps one: it is listed,
//! saved and re-applied until it is reverted, so a source that gets the field back
//! picks the edit up again.
//!
//! An entity with no baseline (loaded while its source file was missing) has only
//! its stored overrides: an edit made then is not tracked until the source is back.
//!
//! The root's position and rotation belong to the instance, never the source (Unity
//! does the same): propagation keeps them and they are never an override.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;

use crate::components::{Entity, TransformComponent};
use crate::ecs::World;
use crate::scene::prefab::overrides::{diff_leaves, leaves};

/// `entity`'s overrides now: its diff against its baseline, or the stored set when it
/// has none yet (a loaded instance whose source was missing). Empty when unlinked.
pub fn live_overrides(entity: &Entity) -> BTreeMap<String, Value> {
    let Some(link) = &entity.prefab_link else {
        return BTreeMap::new();
    };
    let mut out = match &link.baseline {
        Some(base) => {
            let now = leaves(entity);
            let mut out = diff_leaves(&now, base);
            let unused = |path: &String| !now.contains_key(path) && !base.contains_key(path);
            for (path, leaf) in link.overrides.iter().filter(|(p, _)| unused(p)) {
                out.entry(path.clone()).or_insert_with(|| leaf.clone());
            }
            out
        }
        None => link.overrides.clone(),
    };
    if link.local_id == 0 {
        out.retain(|path, _| !is_root_placement(path));
    }
    out
}

/// The live overrides of entity `id` ([`live_overrides`]).
pub fn entity_overrides(world: &World, id: u32) -> BTreeMap<String, Value> {
    world
        .reference_document(id)
        .map(|e| live_overrides(&e))
        .unwrap_or_default()
}

/// Store each of `ids`' live overrides on its link, so the propagation that follows
/// re-applies exactly the edits made since the last one.
pub fn sync_overrides(world: &mut World, ids: &[u32]) {
    for &id in ids {
        let overrides = entity_overrides(world, id);
        if let Some(mut link) = world.prefab_link_mut(id) {
            link.overrides = overrides;
        }
    }
}

/// Write every linked document's live overrides onto its link (a save, a snapshot).
pub fn write_overrides(entities: &mut [Entity]) {
    for entity in entities {
        let overrides = live_overrides(entity);
        if let Some(link) = entity.prefab_link.as_mut() {
            link.overrides = overrides;
        }
    }
}

/// Take each of `ids`' current values as its baseline: a freshly stamped instance is
/// its source exactly.
pub fn take_baselines(world: &mut World, ids: impl IntoIterator<Item = u32>) {
    for id in ids {
        let Some(doc) = world.reference_document(id) else {
            continue;
        };
        if let Some(mut link) = world.prefab_link_mut(id) {
            link.baseline = Some(Arc::new(leaves(&doc)));
        }
    }
}

/// `entity` (a propagation's rebuild) with the root's position and rotation put back
/// from `live`, the instance's transform as it was: the root's placement is never the
/// source's.
pub fn keep_root_placement(entity: &mut Entity, live: Option<&TransformComponent>) {
    let is_root = entity.prefab_link.as_ref().is_some_and(|l| l.local_id == 0);
    if let (true, Some(live)) = (is_root, live) {
        entity.transform.position = live.position;
        entity.transform.rotation = live.rotation;
    }
}

/// Whether override `path` is part of the root's placement (its position or rotation).
fn is_root_placement(path: &str) -> bool {
    ["/transform/position", "/transform/rotation"]
        .iter()
        .any(|p| path == *p || path.starts_with(&format!("{p}/")))
}

#[cfg(test)]
#[path = "live_source_tests.rs"]
mod live_source_tests;
#[cfg(test)]
#[path = "live_tests.rs"]
mod live_tests;
