//! src/scene/prefab/live/mod.rs — an instance edit is an override the moment it is made (#1527).
//!
//! Every entity of a linked instance keeps the source as it last took it
//! ([`PrefabLink::baseline`](crate::components::PrefabLink)): its masked document as
//! the stamp or the last propagation built it, before any override. Its live overrides are its
//! diff against that baseline, so an edit is an override whatever made it (the gizmo,
//! the inspector, a Lua setter, the MCP), with no hook in any setter, as in Unity.
//!
//! The diff is taken against the source as last *propagated*, never the file as it
//! is now: a source changed since would turn every leaf the instance never touched
//! into an override, and the instance would stop tracking the source. So the live
//! set is stored on the link ([`sync_overrides`]) before a propagation re-reads the
//! source, and a save or a Play snapshot writes it ([`write_overrides`]).
//!
//! The live diff *is* the override set: a stored override it no longer contains is
//! gone, so an edit the author undid (a component removed and re-added, an enum
//! switched back) never comes back. So an override on a component the source has
//! since removed is dropped; Unity keeps it as an "unused override", rusty doesn't.
//!
//! A document the World writes (a save, a Play snapshot) carries the overrides and
//! no baseline: its load or restore rebuilds one from the source, or, when the
//! source is missing, leaves it without.
//!
//! An entity with no baseline (loaded while its source file was missing) has only
//! its stored overrides: an edit made then is not tracked until the source is back.
//!
//! The root's position and rotation belong to the instance, never the source (Unity
//! does the same): propagation keeps them and they are never an override.

use std::sync::Arc;

use serde_json::Value;

use crate::components::{Entity, PrefabOverrides, TransformComponent};
use crate::ecs::World;
use crate::scene::prefab::diff::{diff, diff_values, masked};
use crate::scene::prefab::overrides::apply_values;

/// `entity`'s overrides now: its diff against its baseline, or the stored set when it
/// has none yet (a loaded instance whose source was missing). Empty when unlinked.
pub fn live_overrides(entity: &Entity) -> PrefabOverrides {
    let Some(link) = &entity.prefab_link else {
        return PrefabOverrides::default();
    };
    let mut out = match &link.baseline {
        Some(base) => diff(entity, base),
        None => link.overrides.clone(),
    };
    if link.local_id == 0 {
        out.modifications.retain(|path, _| !is_root_placement(path));
    }
    out
}

/// The live overrides of entity `id` ([`live_overrides`]).
pub fn entity_overrides(world: &World, id: u32) -> PrefabOverrides {
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

/// Write every linked document's live overrides onto its link and drop its baseline
/// (a save, a snapshot): what comes back propagates a fresh one, or has none while
/// its source is missing, so a stale one (naming bones a Stop respawns) never
/// outlives the World that took it.
pub fn write_overrides(entities: &mut [Entity]) {
    for entity in entities {
        let overrides = live_overrides(entity);
        if let Some(link) = entity.prefab_link.as_mut() {
            link.overrides = overrides;
            link.baseline = None;
        }
    }
}

/// Move entity `id`'s baseline by what a model re-import changed on it, `before`
/// being its masked document before: a refit mesh collider is the model's, not an
/// edit, so it is neither an override nor applied to the source; every other leaf
/// keeps its override.
pub fn shift_baseline(world: &mut World, id: u32, before: &Value) {
    let Some(after) = world.reference_document(id).map(|doc| masked(&doc)) else {
        return;
    };
    let Some(mut link) = world.prefab_link_mut(id) else {
        return;
    };
    let Some(base) = link.baseline.as_mut().map(Arc::make_mut) else {
        return;
    };
    apply_values(base, &diff_values(before, &after));
}

/// Take each of `ids`' current values as its baseline: a freshly stamped instance is
/// its source exactly.
pub fn take_baselines(world: &mut World, ids: impl IntoIterator<Item = u32>) {
    for id in ids {
        let Some(doc) = world.reference_document(id) else {
            continue;
        };
        if let Some(mut link) = world.prefab_link_mut(id) {
            link.baseline = Some(Arc::new(masked(&doc)));
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
mod component_tests;
#[cfg(test)]
mod list_apply_tests;
#[cfg(test)]
mod list_tests;
#[cfg(test)]
mod size_tests;
#[cfg(test)]
mod source_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod undo_tests;
