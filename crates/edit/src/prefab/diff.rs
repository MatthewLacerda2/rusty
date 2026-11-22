//! An instance's overrides are its diff from its baseline (the source as it last took it), taken on
//! the entity's serde JSON with identity fields masked out (`STRIPPED_KEYS`). Unity's three shapes:
//! added component, removed component, property modification; a list whose length changed carries a
//! size entry and its new elements whole.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use rusty_components::{escape_token, Entity, PrefabOverrides, SIZE_TOKEN};

/// Identity and structure the propagation owns, and the link itself: never diffed or overridden.
const STRIPPED_KEYS: &[&str] = &["id", "parent_id", "children", "prefab_link"];

pub fn masked(entity: &Entity) -> Value {
    let mut value = serde_json::to_value(entity).unwrap_or(Value::Null);
    if let Some(obj) = value.as_object_mut() {
        for key in STRIPPED_KEYS {
            obj.remove(*key);
        }
    }
    value
}

pub fn diff(entity: &Entity, base: &Value) -> PrefabOverrides {
    diff_values(base, &masked(entity))
}

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

fn present(value: Option<&Value>) -> Option<&Value> {
    value.filter(|v| !v.is_null())
}

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
        (_, now) => {
            out.insert(path.to_string(), now.cloned().unwrap_or(Value::Null));
        }
    }
}

/// An enum switched variant (`{"Box": ..}` to `{"Sphere": ..}`) is one entry, applied whole.
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

#[cfg(test)]
pub fn diff_entity(instance: &Entity, baseline: &Entity) -> PrefabOverrides {
    diff(instance, &masked(baseline))
}

#[cfg(test)]
#[path = "diff_tests.rs"]
mod prefab_diff_tests;
