//! src/scene/prefab/diff.rs — the per-instance override diff (#216, #1527).
//!
//! An instance's overrides are its difference from its *baseline*, the source as it
//! last took it (`live/`), taken on the entity's serde JSON with the volatile and
//! identity fields masked out (see `STRIPPED_KEYS`), so ids, parenting, rehydrated
//! GPU geometry and the link itself never count. It covers every first-class
//! component with no per-component code, and emits Unity's three shapes
//! ([`PrefabOverrides`]): a component one side lacks is an added or a removed
//! component, and everything else is property modifications, one per changed leaf,
//! a list whose length changed carrying a size entry and its new elements whole.
//! [`apply_overrides`](super::overrides::apply_overrides) is its inverse.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::components::{escape_token, Entity, PrefabOverrides, SIZE_TOKEN};

/// Top-level entity keys that must never be diffed/overridden: identity + structure
/// the propagation already owns (ids/parenting are remapped per instance), the
/// staged-material slot, and the link itself (an instance's link is set by the
/// stamper, not copied from source).
const STRIPPED_KEYS: &[&str] = &[
    "id",
    "parent_id",
    "children",
    "prefab_link",
    "pending_material",
];

/// Mesh sub-keys that are rehydrated from the reference on load (the heavy GPU
/// buffers + the transient revision). The reference fields (`primitive_type`,
/// `asset_ref`) and authored rig stay, so a *changed mesh reference* is still a
/// diff, but raw vertex soup is not.
const STRIPPED_MESH_KEYS: &[&str] = &[
    "vertices",
    "indices",
    "bind_palette",
    "skin",
    "clips",
    "pose_palette",
    "revision",
];

/// The serde JSON of `entity` with the volatile/identity keys masked out, so the
/// diff sees only authored component data: the form a baseline is kept in.
pub fn masked(entity: &Entity) -> Value {
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

/// The overrides of `entity` against `base`, its baseline ([`masked`]).
pub fn diff(entity: &Entity, base: &Value) -> PrefabOverrides {
    diff_values(base, &masked(entity))
}

/// The overrides that turn masked document `base` into `inst`: a top-level object
/// one side lacks is an added or a removed component; everything else is walked
/// into property modifications.
pub fn diff_values(base: &Value, inst: &Value) -> PrefabOverrides {
    let mut out = PrefabOverrides::default();
    let none = Map::new();
    let b = base.as_object().unwrap_or(&none);
    let i = inst.as_object().unwrap_or(&none);
    for key in b.keys().chain(i.keys()).collect::<BTreeSet<_>>() {
        let path = format!("/{}", escape_token(key));
        match (present(b.get(key)), present(i.get(key))) {
            (None, Some(v)) if v.is_object() => {
                out.added_components.insert(key.clone(), v.clone());
            }
            (Some(v), None) if v.is_object() => {
                out.removed_components.insert(key.clone());
            }
            (was, now) => walk(&path, was, now, &mut out.modifications),
        }
    }
    out
}

/// A value that is there: neither missing nor `null`.
fn present(value: Option<&Value>) -> Option<&Value> {
    value.filter(|v| !v.is_null())
}

/// Diff `inst` against `base`, both the value at `path` (`None` when absent), into
/// property modifications.
fn walk(path: &str, base: Option<&Value>, inst: Option<&Value>, out: &mut BTreeMap<String, Value>) {
    let empty = Value::Array(Vec::new());
    match (base, inst) {
        (None, None) => {}
        (Some(b), Some(i)) if b == i => {}
        (Some(Value::Object(b)), Some(Value::Object(i))) if !swaps_variant(b, i) => {
            for key in b.keys().chain(i.keys()).collect::<BTreeSet<_>>() {
                let p = format!("{path}/{}", escape_token(key));
                walk(&p, present(b.get(key)), present(i.get(key)), out);
            }
        }
        // A list one side lacks is an empty one, so it diffs element by element.
        (Some(Value::Array(_)), None) => walk(path, base, Some(&empty), out),
        (None, Some(Value::Array(_))) => walk(path, Some(&empty), inst, out),
        (Some(Value::Array(b)), Some(Value::Array(i))) => {
            if b.len() != i.len() {
                out.insert(format!("{path}/{SIZE_TOKEN}"), Value::from(i.len()));
            }
            for (n, now) in i.iter().enumerate() {
                let p = format!("{path}/{n}");
                match b.get(n) {
                    Some(was) => walk(&p, Some(was), Some(now), out),
                    None => {
                        out.insert(p, now.clone());
                    }
                }
            }
        }
        // A changed scalar, a value the base lacks (whole), or one the instance
        // dropped (`null`).
        (_, now) => {
            out.insert(path.to_string(), now.cloned().unwrap_or(Value::Null));
        }
    }
}

/// Whether `inst` trades a key of `base` for another (an enum switched variant,
/// `{"Box": ..}` to `{"Sphere": ..}`): one entry, applied whole.
fn swaps_variant(base: &Map<String, Value>, inst: &Map<String, Value>) -> bool {
    let keys = |m: &Map<String, Value>| -> BTreeSet<String> {
        m.iter()
            .filter(|(_, v)| !v.is_null())
            .map(|(k, _)| k.clone())
            .collect()
    };
    let (b, i) = (keys(base), keys(inst));
    i.difference(&b).next().is_some() && b.difference(&i).next().is_some()
}

/// [`diff`] of two whole entities, for the round-trip tests.
#[cfg(test)]
pub fn diff_entity(instance: &Entity, baseline: &Entity) -> PrefabOverrides {
    diff(instance, &masked(baseline))
}
