//! src/scene/prefab/diff.rs — the generic per-instance override diff (#216, #1527).
//!
//! An instance's overrides are its difference from its *baseline*, the source as it
//! last took it (`live.rs`): a flat map of RFC-6901 `json-pointer-path -> value`,
//! taken on the entity's serde JSON with the volatile/identity fields masked out
//! (see `STRIPPED_KEYS`), so ids, parenting, rehydrated GPU geometry and the link
//! itself never count. It covers every first-class component with no per-component
//! code. A changed scalar is one leaf; anything the baseline did not have is one
//! entry holding its whole value (a component added to the instance, a key, a list
//! that grew), and anything the instance dropped is one `null` at the highest path
//! it dropped (a removed component is `/camera: null`), so
//! [`apply_overrides`](super::overrides::apply_overrides) can rebuild each, as Unity
//! keeps an *added component* whole.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::components::{escape_token, Entity};

/// Json-pointer leaves: a baseline, or an override map.
pub type Leaves = BTreeMap<String, Value>;

/// Top-level entity keys that must never be diffed/overridden: identity + structure
/// the propagation already owns (ids/parenting are remapped per instance), the
/// rehydrated-on-load mesh geometry (stored only as a reference; re-derived, so it
/// would otherwise show as a giant spurious diff), the staged-material slot, and the
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

/// Lists overridden whole, as one leaf, rather than per element: an entity's material
/// slots follow its mesh's runs (#1285), so an override may add or lengthen the list,
/// which per-element leaves cannot (the apply never grows an array).
const WHOLE_LISTS: &[&str] = &["/material/materials"];

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
/// arrays recurse (arrays index by position, but a [`WHOLE_LISTS`] list is one
/// leaf); every scalar — and `null` — is a leaf,
/// so a present↔absent component (`Some`↔`None`, i.e. object↔`null`) is captured.
fn flatten(prefix: &str, value: &Value, out: &mut Leaves) {
    match value {
        Value::Object(map) if !map.is_empty() => {
            for (k, v) in map {
                flatten(&format!("{prefix}/{}", escape_token(k)), v, out);
            }
        }
        Value::Array(items) if !items.is_empty() && !WHOLE_LISTS.contains(&prefix) => {
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

/// `entity`'s authored data as json-pointer leaves, identity and geometry masked:
/// the form a baseline is kept in and an override map is diffed from.
pub fn leaves(entity: &Entity) -> Leaves {
    let mut out = BTreeMap::new();
    flatten("", &maskable_json(entity), &mut out);
    out
}

/// [`diff`] of two whole entities, for the round-trip tests.
#[cfg(test)]
pub fn diff_entity(instance: &Entity, baseline: &Entity) -> Leaves {
    diff(instance, &leaves(baseline))
}

/// The override map of `entity` against `base`: every changed leaf, every subtree
/// the baseline lacks as one entry, and every path the entity dropped as one `null`.
/// The entries never nest, so applying them in any order rebuilds the same entity.
pub fn diff(entity: &Entity, base: &Leaves) -> Leaves {
    let json = maskable_json(entity);
    let mut out = Leaves::new();
    walk("", &json, base, &mut out);
    for path in base.keys() {
        if json.pointer(path).is_none() && !under_an_entry(&out, path) {
            out.insert(path.clone(), Value::Null);
        }
    }
    out
}

/// Diff `inst`, the entity's value at `path`, into `out` against the baseline.
fn walk(path: &str, inst: &Value, base: &Leaves, out: &mut Leaves) {
    if let Some(was) = base.get(path) {
        // A leaf in the baseline: the same value is no edit; anything else is one
        // entry, a subtree where it had `null` (an added component) included.
        if was != inst {
            out.insert(path.to_string(), inst.clone());
        }
        return;
    }
    if !has_under(base, path) {
        out.insert(path.to_string(), inst.clone());
        return;
    }
    match inst {
        Value::Object(map) if !map.is_empty() => {
            if swaps_keys(path, map, base) {
                out.insert(path.to_string(), inst.clone());
                return;
            }
            for (k, v) in map {
                walk(&format!("{path}/{}", escape_token(k)), v, base, out);
            }
        }
        // A list that grew is one entry: the apply never grows an array.
        Value::Array(items) if !items.is_empty() && !WHOLE_LISTS.contains(&path) => {
            let slot = |i: usize| format!("{path}/{i}");
            let held = |p: &str| base.contains_key(p) || has_under(base, p);
            if (0..items.len()).all(|i| held(&slot(i))) {
                for (i, v) in items.iter().enumerate() {
                    walk(&slot(i), v, base, out);
                }
            } else {
                out.insert(path.to_string(), inst.clone());
            }
        }
        // A leaf where the baseline has a subtree (a removed component is `null`).
        leaf => {
            out.insert(path.to_string(), leaf.clone());
        }
    }
}

/// Whether `map` trades a key of the baseline's object at `path` for another (an
/// enum switched variant, `{"Box": ..}` to `{"Sphere": ..}`): one entry, applied whole.
fn swaps_keys(path: &str, map: &serde_json::Map<String, Value>, base: &Leaves) -> bool {
    let prefix = format!("{path}/");
    let keys: Vec<String> = map.keys().map(|k| escape_token(k)).collect();
    let gained = keys.iter().any(|k| {
        let p = format!("{prefix}{k}");
        !base.contains_key(&p) && !has_under(base, &p)
    });
    let lost = base
        .range(prefix.clone()..)
        .map(|(k, _)| k)
        .take_while(|k| k.starts_with(&prefix))
        .any(|k| {
            let token = k[prefix.len()..].split('/').next().unwrap_or_default();
            !keys.iter().any(|key| key == token)
        });
    gained && lost
}

/// Whether `base` has a leaf strictly under `path`.
fn has_under(base: &Leaves, path: &str) -> bool {
    let prefix = format!("{path}/");
    base.range(prefix.clone()..)
        .next()
        .is_some_and(|(k, _)| k.starts_with(&prefix))
}

/// Whether `path` or one of its ancestors is already an entry of `out`.
fn under_an_entry(out: &Leaves, path: &str) -> bool {
    out.contains_key(path)
        || path
            .match_indices('/')
            .any(|(i, _)| i > 0 && out.contains_key(&path[..i]))
}
