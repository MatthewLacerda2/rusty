//! The diff is taken against the source as last *propagated*, never the file as it is now: a source changed since would turn every leaf the instance never touched into an override.
//! The live diff *is* the override set: an edit the author undid never comes back, and an override on a component the source removed is dropped (Unity keeps an "unused override"; rusty does not).
//! The root's position and rotation belong to the instance, never the source (as Unity): never an override.

use std::sync::Arc;

use serde_json::Value;

use crate::prefab::diff::{diff, diff_values, masked};
use crate::prefab::overrides::apply_values;
use rusty_components::{Entity, PrefabOverrides, TransformComponent};
use rusty_ecs::World;

pub fn live_overrides(entity: &Entity) -> PrefabOverrides {
    let Some(link) = &entity.prefab_link else {
        return PrefabOverrides::default();
    };
    let mut out = match &link.baseline {
        Some(base) => diff(entity, base),
        None => link.overrides.clone(),
    };
    out.removed_parts = link.overrides.removed_parts.clone();
    if link.local_id == 0 {
        out.modifications.retain(|path, _| !is_root_placement(path));
    }
    out
}

pub fn entity_overrides(world: &World, id: u32) -> PrefabOverrides {
    world
        .reference_document(id)
        .map(|e| live_overrides(&e))
        .unwrap_or_default()
}

pub fn sync_overrides(world: &mut World, ids: &[u32]) {
    for &id in ids {
        let overrides = entity_overrides(world, id);
        if let Some(mut link) = world.prefab_link_mut(id) {
            link.overrides = overrides;
        }
    }
}

pub fn write_overrides(entities: &mut [Entity]) {
    for entity in entities {
        let overrides = live_overrides(entity);
        if let Some(link) = entity.prefab_link.as_mut() {
            link.overrides = overrides;
            link.baseline = None;
        }
    }
}

/// A refit mesh collider is the model's change, not an edit: never an override.
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

pub fn keep_root_placement(entity: &mut Entity, live: Option<&TransformComponent>) {
    let is_root = entity.prefab_link.as_ref().is_some_and(|l| l.local_id == 0);
    if let (true, Some(live)) = (is_root, live) {
        entity.transform.position = live.position;
        entity.transform.rotation = live.rotation;
    }
}

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
