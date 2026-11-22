//! The one walk over every entity id a prefab instance's overrides hold: in a modification's
//! leaf, inside its whole value, or through an added component's own typed walk.

use serde::Deserialize;
use serde_json::Value;

use super::prefab_link::PrefabOverrides;
use super::refs::{escape_token, EntityRefs, RefVisit};
use super::Entity;
use crate::with_ref_components;

/// A dropped id becomes `null`, the override of no reference, except in a list of ids, which it
/// leaves, as the typed walk does.
pub fn visit_override_refs(overrides: &mut PrefabOverrides, visit: &mut RefVisit<'_>) {
    for (path, value) in &mut overrides.modifications {
        visit_value_refs(path, value, visit);
    }
    for (name, value) in &mut overrides.added_components {
        if !visit_component_refs(name, value, visit) {
            visit_value_refs(&format!("/{}", escape_token(name)), value, visit);
        }
    }
}

pub fn each_override_ref(overrides: &PrefabOverrides, visit: &mut dyn FnMut(&str, u32)) {
    for (path, value) in &overrides.modifications {
        each_value_ref(path, value, visit);
    }
    for (name, value) in &overrides.added_components {
        if !each_component_ref(name, value, visit) {
            each_value_ref(&format!("/{}", escape_token(name)), value, visit);
        }
    }
}

fn visit_value_refs(pointer: &str, value: &mut Value, visit: &mut RefVisit<'_>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                visit_value_refs(&format!("{pointer}/{}", escape_token(k)), v, visit);
            }
        }
        Value::Array(items) => {
            let mut n = 0;
            items.retain_mut(|v| {
                let p = format!("{pointer}/{n}");
                n += 1;
                visit_value_refs(&p, v, visit);
                !(v.is_null() && Entity::is_ref_list_pointer(&p))
            });
        }
        leaf if Entity::is_ref_pointer(pointer) => {
            let mut slot = match &*leaf {
                Value::Null => None,
                v => match id_of(v) {
                    Some(id) => Some(id),
                    None => return,
                },
            };
            visit(pointer, &mut slot);
            *leaf = slot.map_or(Value::Null, Value::from);
        }
        _ => {}
    }
}

fn each_value_ref(pointer: &str, value: &Value, visit: &mut dyn FnMut(&str, u32)) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                each_value_ref(&format!("{pointer}/{}", escape_token(k)), v, visit);
            }
        }
        Value::Array(items) => {
            for (i, v) in items.iter().enumerate() {
                each_value_ref(&format!("{pointer}/{i}"), v, visit);
            }
        }
        leaf if Entity::is_ref_pointer(pointer) => {
            if let Some(id) = id_of(leaf) {
                visit(pointer, id);
            }
        }
        _ => {}
    }
}

fn id_of(leaf: &Value) -> Option<u32> {
    leaf.as_u64().and_then(|n| u32::try_from(n).ok())
}

macro_rules! component_walks {
    ($($field:ident: $ty:ty,)*) => {
        // `false`, `value` untouched, when `name`'s kind holds no ids or `value` doesn't read
        // back as it.
        fn visit_component_refs(name: &str, value: &mut Value, visit: &mut RefVisit<'_>) -> bool {
            match name {
                $(stringify!($field) => {
                    let Ok(mut c) = <$ty>::deserialize(&*value) else {
                        return false;
                    };
                    let prefix = concat!("/", stringify!($field));
                    c.visit_refs(&mut |p, r| visit(&format!("{prefix}{p}"), r));
                    match serde_json::to_value(&c) {
                        Ok(walked) => {
                            *value = walked;
                            true
                        }
                        Err(_) => false,
                    }
                })*
                _ => false,
            }
        }

        fn each_component_ref(name: &str, value: &Value, visit: &mut dyn FnMut(&str, u32)) -> bool {
            match name {
                $(stringify!($field) => {
                    let Ok(c) = <$ty>::deserialize(value) else {
                        return false;
                    };
                    let prefix = concat!("/", stringify!($field));
                    c.each_ref(&mut |p, r| visit(&format!("{prefix}{p}"), r));
                    true
                })*
                _ => false,
            }
        }
    };
}
with_ref_components!(component_walks);
