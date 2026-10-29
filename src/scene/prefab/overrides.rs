//! src/scene/prefab/overrides.rs — applying a per-instance override map (#216).
//!
//! A linked prefab instance's divergence from its source is a GENERIC field diff
//! (`diff.rs`): a flat map of `json-pointer-path -> value` against its *baseline*,
//! the source as it last took it (`live.rs`, #1527), with **no per-component code**.
//! [`apply_overrides`] is its inverse: a fresh source entity plus the map rebuilds
//! the instance. Propagation is "rebuild from the source, then re-apply the map on
//! top"; nothing diffs against the source as it is now, which would make every leaf
//! it changed an override.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::components::Entity;
#[cfg(test)]
use crate::scene::prefab::diff::diff_entity;

/// Apply an override map onto `baseline` in place, leaf by leaf: set each path's
/// leaf on the entity's JSON form, then deserialize back. Identity/structure/geometry
/// keys are preserved from `baseline` because the diff never carries them. A path
/// that no longer resolves (the source's shape changed underneath an override) is
/// skipped. A `null` written into an array element the element type can't hold (a
/// destroyed LOD renderer's id, a shortened list) drops that element. A leaf that
/// still doesn't fit is skipped with a warning naming it, and the entity keeps every
/// other override: one stale leaf never costs the rest.
pub fn apply_overrides(baseline: &mut Entity, overrides: &BTreeMap<String, Value>) {
    if overrides.is_empty() {
        return;
    }
    let Ok(mut value) = serde_json::to_value(&*baseline) else {
        return;
    };
    let (mut drops, plain): (Vec<_>, Vec<_>) = overrides
        .iter()
        .partition(|(path, leaf)| leaf.is_null() && in_array(&value, path));
    let mut all = value.clone();
    for (path, leaf) in &plain {
        set_pointer(&mut all, path, (*leaf).clone());
    }
    if fits(&all) {
        value = all;
    } else {
        for (path, leaf) in plain {
            let mut trial = value.clone();
            set_pointer(&mut trial, path, leaf.clone());
            keep_if_fits(&mut value, trial, &baseline.name, path);
        }
    }
    // Highest index first, so dropping one element never shifts a later drop's.
    drops.sort_by_key(|(path, _)| std::cmp::Reverse(array_slot(path)));
    for (path, _) in drops {
        let mut trial = value.clone();
        set_pointer(&mut trial, path, Value::Null);
        if fits(&trial) {
            value = trial;
            continue;
        }
        let mut trial = value.clone();
        if let Some((parent, Some(i))) = array_slot(path) {
            if let Some(Value::Array(items)) = trial.pointer_mut(&parent) {
                if i < items.len() {
                    items.remove(i);
                }
            }
        }
        keep_if_fits(&mut value, trial, &baseline.name, path);
    }
    if let Ok(rebuilt) = serde_json::from_value::<Entity>(value) {
        let (id, parent_id, children) =
            (baseline.id, baseline.parent_id, baseline.children.clone());
        let link = baseline.prefab_link.clone();
        *baseline = rebuilt;
        // Identity/structure/link are the stamper's, not the override map's.
        baseline.id = id;
        baseline.parent_id = parent_id;
        baseline.children = children;
        baseline.prefab_link = link;
    }
}

/// Whether `value` reads back as an entity.
fn fits(value: &Value) -> bool {
    serde_json::from_value::<Entity>(value.clone()).is_ok()
}

/// Take `trial` when it still reads back as an entity; otherwise warn that override
/// `path` on entity `name` was skipped.
fn keep_if_fits(value: &mut Value, trial: Value, name: &str, path: &str) {
    if fits(&trial) {
        *value = trial;
    } else {
        log::warn!(
            "[Prefab] entity '{name}': override '{path}' no longer fits the prefab's \
             source; it was skipped, and the entity's other overrides kept."
        );
    }
}

/// The parent pointer of `path` and its last token as an index.
fn array_slot(path: &str) -> Option<(String, Option<usize>)> {
    let (parent, last) = path.rsplit_once('/')?;
    Some((parent.to_string(), last.parse().ok()))
}

/// Whether `path`'s leaf is an element of an array in `value`.
fn in_array(value: &Value, path: &str) -> bool {
    array_slot(path).is_some_and(|(parent, i)| {
        i.is_some() && value.pointer(&parent).is_some_and(Value::is_array)
    })
}

/// Set `value` at an RFC-6901 `pointer` to `leaf`, creating intermediate objects as
/// needed. Array growth is not supported (overrides only edit existing array slots,
/// e.g. a vec3 component); an out-of-range array index is a no-op.
fn set_pointer(value: &mut Value, pointer: &str, leaf: Value) {
    let tokens: Vec<String> = pointer
        .split('/')
        .skip(1) // leading "" before the first "/"
        .map(unescape)
        .collect();
    let Some((last, parents)) = tokens.split_last() else {
        return;
    };
    let mut cursor = value;
    for token in parents {
        cursor = match descend(cursor, token) {
            Some(next) => next,
            None => return,
        };
    }
    place(cursor, last, leaf);
}

/// Step one token into a container, materializing a missing object key as an empty
/// object so a deep override path can be created. A child slot that is currently
/// `null` (an absent component being *added* by the override) is promoted to an empty
/// object first, so paths like `/light/intensity` can be built over a `None`
/// baseline. Returns `None` for an out-of-range array index (we never grow arrays).
fn descend<'a>(cursor: &'a mut Value, token: &str) -> Option<&'a mut Value> {
    match cursor {
        Value::Object(map) => {
            let slot = map
                .entry(token.to_string())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            if slot.is_null() {
                *slot = Value::Object(serde_json::Map::new());
            }
            Some(slot)
        }
        Value::Array(items) => token
            .parse::<usize>()
            .ok()
            .and_then(move |i| items.get_mut(i)),
        _ => None,
    }
}

/// Write the final leaf into its parent container under `token`.
fn place(cursor: &mut Value, token: &str, leaf: Value) {
    match cursor {
        Value::Object(map) => {
            map.insert(token.to_string(), leaf);
        }
        Value::Array(items) => {
            if let Some(slot) = token.parse::<usize>().ok().and_then(|i| items.get_mut(i)) {
                *slot = leaf;
            }
        }
        _ => {}
    }
}

/// Inverse of [`escape_token`](crate::components::escape_token) (RFC 6901: `~1` -> `/`, `~0` -> `~`, in that order).
fn unescape(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}

#[cfg(test)]
#[path = "overrides_tests.rs"]
mod prefab_overrides_tests;

#[cfg(test)]
#[path = "material_list_override_tests.rs"]
mod material_list_override_tests;
