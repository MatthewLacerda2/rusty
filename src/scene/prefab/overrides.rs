//! src/scene/prefab/overrides.rs — applying a per-instance override set (#216, #1527).
//!
//! A linked prefab instance's divergence from its source is a [`PrefabOverrides`]
//! (`diff.rs`) against its *baseline*, the source as it last took it (`live/`), with
//! **no per-component code**. [`apply_overrides`] is its inverse: a fresh source
//! entity plus the overrides rebuilds the instance. Propagation is "rebuild from the
//! source, then re-apply the overrides on top"; nothing diffs against the source as
//! it is now, which would make every leaf it changed an override.
//!
//! The order is Unity's: removed components go, added components come whole, each
//! list takes its size (shallowest first), then each modification sets its leaf. A
//! slot a list grew by that no modification fills is dropped again.

use serde_json::Value;

use super::pointer::set_pointer;
use crate::components::{Entity, PrefabOverrides, SIZE_TOKEN};

/// One step of applying an override set onto an entity's JSON form.
enum Op<'a> {
    Remove(&'a str),
    Add(&'a str, &'a Value),
    Size(&'a str, usize),
    Set(&'a str, &'a Value),
}

/// Apply `overrides` onto `baseline` in place, then deserialize back. Identity,
/// structure and the link are kept from `baseline` (the diff never carries them). A
/// path that no longer resolves (the source's shape changed underneath an override)
/// is skipped. A `null` written into an array element the element type can't hold
/// (a dropped LOD renderer's id) drops that element. An override that still doesn't
/// fit is skipped with a warning naming it, and the entity keeps every other one.
pub fn apply_overrides(baseline: &mut Entity, overrides: &PrefabOverrides) {
    if overrides.is_empty() {
        return;
    }
    let Ok(mut value) = serde_json::to_value(&*baseline) else {
        return;
    };
    apply_json(&mut value, overrides, &baseline.name);
    if let Ok(rebuilt) = serde_json::from_value::<Entity>(value) {
        let (id, parent_id, children) =
            (baseline.id, baseline.parent_id, baseline.children.clone());
        let link = baseline.prefab_link.clone();
        *baseline = rebuilt;
        // Identity/structure/link are the stamper's, not the overrides'.
        baseline.id = id;
        baseline.parent_id = parent_id;
        baseline.children = children;
        baseline.prefab_link = link;
    }
}

/// Apply `overrides` onto a masked document (`diff::masked`) as they are, with no
/// fit check: how a baseline moves by a change that is not an override.
pub fn apply_values(value: &mut Value, overrides: &PrefabOverrides) {
    let mut grown = Vec::new();
    for op in &ops(value, overrides) {
        run(value, op, &mut grown);
    }
    *value = finished(value, &grown, |p| overrides.modifications.contains_key(p));
}

/// [`apply_overrides`] on an entity's JSON form, `name` naming it in a warning.
fn apply_json(value: &mut Value, overrides: &PrefabOverrides, name: &str) {
    let ops = ops(value, overrides);
    let mut all = value.clone();
    let mut grown = Vec::new();
    for op in &ops {
        run(&mut all, op, &mut grown);
    }
    let all = finished(&all, &grown, |p| overrides.modifications.contains_key(p));
    if fits(&all) {
        *value = all;
        return;
    }
    // One at a time: an override that doesn't fit is skipped, the rest kept.
    let mut filled: Vec<String> = Vec::new();
    let mut grown = Vec::new();
    for op in &ops {
        let mut trial = value.clone();
        let mut trial_grown = grown.clone();
        run(&mut trial, op, &mut trial_grown);
        let mut trial_filled = filled.clone();
        if let Op::Set(path, _) = op {
            trial_filled.push(path.to_string());
        }
        let is_filled = |p: &str| trial_filled.iter().any(|f| f == p);
        let ok = fits(&finished(&trial, &trial_grown, is_filled))
            || matches!(op, Op::Set(path, Value::Null) if drop_element(&mut trial, path)
                && fits(&finished(&trial, &trial_grown, is_filled)));
        if ok {
            (*value, grown, filled) = (trial, trial_grown, trial_filled);
        } else {
            log::warn!(
                "[Prefab] entity '{name}': override '{}' no longer fits the prefab's \
                 source; it was skipped, and the entity's other overrides kept.",
                op_key(op)
            );
        }
    }
    let is_filled = |p: &str| filled.iter().any(|f| f == p);
    *value = finished(value, &grown, is_filled);
}

/// The steps of `overrides` in the order they apply. Sizes go shallowest first, and a
/// `null` into an array element goes last, highest index first, so dropping one
/// element never shifts another's slot.
fn ops<'a>(value: &Value, overrides: &'a PrefabOverrides) -> Vec<Op<'a>> {
    let mut ops: Vec<Op> = overrides
        .removed_components
        .iter()
        .map(|c| Op::Remove(c))
        .collect();
    ops.extend(
        overrides
            .added_components
            .iter()
            .map(|(c, v)| Op::Add(c, v)),
    );
    let (sizes, sets): (Vec<_>, Vec<_>) = overrides
        .modifications
        .iter()
        .partition(|(path, _)| list_of_size(path).is_some());
    let mut sizes: Vec<_> = sizes
        .into_iter()
        .filter_map(|(path, n)| Some((list_of_size(path)?, sane_size(path, n)?)))
        .collect();
    sizes.sort_by_key(|(list, _)| list.matches('/').count());
    ops.extend(sizes.into_iter().map(|(list, n)| Op::Size(list, n)));
    let (mut drops, plain): (Vec<_>, Vec<_>) = sets
        .into_iter()
        .partition(|(path, leaf)| leaf.is_null() && in_array(value, path));
    ops.extend(plain.into_iter().map(|(p, v)| Op::Set(p, v)));
    drops.sort_by_key(|(path, _)| std::cmp::Reverse(array_slot(path)));
    ops.extend(drops.into_iter().map(|(p, v)| Op::Set(p, v)));
    ops
}

/// Run one step on `value`; a list grown records each new slot's pointer in `grown`.
fn run(value: &mut Value, op: &Op, grown: &mut Vec<String>) {
    match *op {
        Op::Remove(c) => {
            if let Some(slot) = value.get_mut(c) {
                *slot = Value::Null;
            }
        }
        Op::Add(c, v) => {
            if let Some(obj) = value.as_object_mut() {
                obj.insert(c.to_string(), v.clone());
            }
        }
        Op::Size(list, n) => {
            // A list the baseline omits (`skip_serializing_if = "Vec::is_empty"`) is
            // an empty one, made where its parent is there to hold it.
            let parent = list.rsplit_once('/').map_or("", |(p, _)| p);
            let missing = value.pointer(list).is_none_or(Value::is_null);
            if missing && value.pointer(parent).is_some_and(Value::is_object) {
                set_pointer(value, list, Value::Array(Vec::new()));
            }
            if let Some(Value::Array(items)) = value.pointer_mut(list) {
                grown.extend((items.len()..n).map(|i| format!("{list}/{i}")));
                items.resize(n, Value::Null);
            }
        }
        Op::Set(path, leaf) => set_pointer(value, path, leaf.clone()),
    }
}

/// `value` with every slot in `grown` that no modification `filled` dropped again,
/// highest index first.
fn finished(value: &Value, grown: &[String], filled: impl Fn(&str) -> bool) -> Value {
    let mut value = value.clone();
    for slot in grown.iter().rev().filter(|s| !filled(s)) {
        drop_element(&mut value, slot);
    }
    value
}

/// The key of the override `op` came from, for a warning.
fn op_key(op: &Op) -> String {
    match *op {
        Op::Remove(c) => format!("-/{c}"),
        Op::Add(c, _) => format!("+/{c}"),
        Op::Size(list, _) => format!("{list}/{SIZE_TOKEN}"),
        Op::Set(path, _) => path.to_string(),
    }
}

/// The longest a size entry may make a list: a larger one is no authored edit (a
/// hand-edited or corrupt scene), and resizing to it would allocate that many slots.
const MAX_LIST_SIZE: u64 = 65_536;

/// Size entry `path`'s length, or `None` (with a warning when it is too large).
fn sane_size(path: &str, n: &Value) -> Option<usize> {
    let n = n.as_u64()?;
    if n > MAX_LIST_SIZE {
        log::warn!(
            "[Prefab] override '{path}' sets a list's length to {n}, over the cap of \
             {MAX_LIST_SIZE}; it was skipped."
        );
        return None;
    }
    usize::try_from(n).ok()
}

/// The list pointer of a size entry `path` (`/scripts/#size` → `/scripts`).
fn list_of_size(path: &str) -> Option<&str> {
    path.strip_suffix(SIZE_TOKEN)?.strip_suffix('/')
}

/// Whether `value` reads back as an entity.
fn fits(value: &Value) -> bool {
    serde_json::from_value::<Entity>(value.clone()).is_ok()
}

/// Remove the array element at `path` from `value`; whether there was one.
fn drop_element(value: &mut Value, path: &str) -> bool {
    let Some((parent, Some(i))) = array_slot(path) else {
        return false;
    };
    match value.pointer_mut(&parent) {
        Some(Value::Array(items)) if i < items.len() => {
            items.remove(i);
            true
        }
        _ => false,
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

#[cfg(test)]
#[path = "overrides_tests.rs"]
mod prefab_overrides_tests;

#[cfg(test)]
#[path = "material_list_override_tests.rs"]
mod material_list_override_tests;
