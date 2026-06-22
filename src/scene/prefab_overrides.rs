//! src/scene/prefab_overrides.rs — generic per-instance override diff (#216).
//!
//! A linked prefab instance records its divergence from the source as a GENERIC
//! field diff, not a typed-per-component struct: a flat map of `json-pointer-path ->
//! value` for every leaf where the instance differs from a freshly-instantiated copy
//! of the source entity. Reusing the serde-value approach `prefab.rs` already uses
//! for material equality, this auto-covers every present and future first-class
//! component with **no per-component code**, and covers component add/remove too — a
//! `None`↔`Some` at a component path is just another diff leaf.
//!
//! The two verbs are symmetric:
//!   - [`diff_entity`] (instance vs. baseline) -> the override map.
//!   - [`apply_overrides`] (baseline + override map) -> the instance.
//! so `apply_overrides(baseline, diff_entity(instance, baseline))` reproduces the
//! instance, and propagation is "rebuild from a fresh baseline, then re-apply the
//! recorded map on top".
//!
//! Leaves are compared on the entity's serde JSON form with the volatile/identity
//! fields masked out (see [`STRIPPED_KEYS`]), so ids, parenting, rehydrated GPU
//! geometry, and the link itself never count as overrides.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::components::Entity;

/// Top-level entity keys that must never be diffed/overridden: identity + structure
/// the propagation already owns (ids/parenting are remapped per instance), the
/// rehydrated-on-load mesh geometry (stored only as a reference; re-derived, so it
/// would otherwise show as a giant spurious diff), the migration carrier, and the
/// link itself (an instance's link is set by the stamper, not copied from source).
const STRIPPED_KEYS: &[&str] = &[
    "id",
    "parent_id",
    "children",
    "prefab_link",
    "pending_material",
];

/// Mesh sub-keys that are rehydrated from the reference on load (the heavy GPU
/// buffers + transient dirty flag). The reference fields (`primitive_type`,
/// `asset_ref`) and authored rig stay, so a *changed mesh reference* is still a
/// diff, but raw vertex soup is not.
const STRIPPED_MESH_KEYS: &[&str] = &[
    "vertices",
    "indices",
    "bind_palette",
    "skin",
    "clips",
    "pose_palette",
    "is_dirty",
];

/// The serde JSON of `entity` with the volatile/identity keys masked out, so the
/// diff sees only authored component data.
fn maskable_json(entity: &Entity) -> Value {
    let mut value = serde_json::to_value(entity).unwrap_or(Value::Null);
    if let Some(obj) = value.as_object_mut() {
        for key in STRIPPED_KEYS {
            obj.remove(*key);
        }
        if let Some(Value::Object(mesh)) = obj.get_mut("mesh") {
            for key in STRIPPED_MESH_KEYS {
                mesh.remove(*key);
            }
        }
    }
    value
}

/// Flatten a JSON value into `out` as RFC-6901 json-pointer leaves. Objects and
/// arrays recurse (arrays index by position); every scalar — and `null` — is a leaf,
/// so a present↔absent component (`Some`↔`None`, i.e. object↔`null`) is captured.
fn flatten(prefix: &str, value: &Value, out: &mut BTreeMap<String, Value>) {
    match value {
        Value::Object(map) if !map.is_empty() => {
            for (k, v) in map {
                flatten(&format!("{prefix}/{}", escape(k)), v, out);
            }
        }
        Value::Array(items) if !items.is_empty() => {
            for (i, v) in items.iter().enumerate() {
                flatten(&format!("{prefix}/{i}"), v, out);
            }
        }
        // A scalar, an empty object, or an empty array is a leaf in its own right.
        leaf => {
            out.insert(prefix.to_string(), leaf.clone());
        }
    }
}

/// Escape a json-pointer reference token (RFC 6901: `~` -> `~0`, `/` -> `~1`).
fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// The override map for `instance` against `baseline`: every flattened leaf whose
/// value differs (a changed scalar, an added leaf, or a removed leaf surfaced as
/// `null`). Both sides are masked first, so identity/geometry never leak in.
pub fn diff_entity(instance: &Entity, baseline: &Entity) -> BTreeMap<String, Value> {
    let mut inst = BTreeMap::new();
    let mut base = BTreeMap::new();
    flatten("", &maskable_json(instance), &mut inst);
    flatten("", &maskable_json(baseline), &mut base);

    let mut diff = BTreeMap::new();
    for (path, inst_val) in &inst {
        if base.get(path) != Some(inst_val) {
            diff.insert(path.clone(), inst_val.clone());
        }
    }
    // A leaf present in the baseline but gone from the instance (e.g. a removed
    // component, or a shortened array) is an override to `null`.
    for path in base.keys() {
        if !inst.contains_key(path) {
            diff.insert(path.clone(), Value::Null);
        }
    }
    diff
}

/// Apply an override map onto `baseline` in place: set each path's leaf on the
/// entity's JSON form, then deserialize back. Identity/structure/geometry keys are
/// preserved from `baseline` because the diff never carries them. A path that no
/// longer resolves (the source's shape changed underneath an override) is skipped,
/// so a stale override can never corrupt the rebuilt entity.
pub fn apply_overrides(baseline: &mut Entity, overrides: &BTreeMap<String, Value>) {
    if overrides.is_empty() {
        return;
    }
    let mut value = match serde_json::to_value(&*baseline) {
        Ok(v) => v,
        Err(_) => return,
    };
    for (path, leaf) in overrides {
        set_pointer(&mut value, path, leaf.clone());
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
/// object so a deep override path can be created. Returns `None` for an out-of-range
/// array index (we never grow arrays).
fn descend<'a>(cursor: &'a mut Value, token: &str) -> Option<&'a mut Value> {
    match cursor {
        Value::Object(map) => Some(
            map.entry(token.to_string())
                .or_insert(Value::Object(serde_json::Map::new())),
        ),
        Value::Array(items) => token.parse::<usize>().ok().and_then(move |i| items.get_mut(i)),
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

/// Inverse of [`escape`] (RFC 6901: `~1` -> `/`, `~0` -> `~`, in that order).
fn unescape(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}

#[cfg(test)]
#[path = "prefab_overrides_tests.rs"]
mod prefab_overrides_tests;
