//! src/scene/authoring/fields/hold.rs — a loaded component's numbers through their
//! ops, once (#1418).
//!
//! The authoring ops hold every bound a field has (`sanitize`, #1361), but only on
//! a write. A scene or prefab file reaches the World without one, so a hand-edited
//! `volume: 4` would play at 4×. [`hold_entity`] closes that at the source: when a
//! scene or prefab is applied, every numeric field (a number, or a vector, colour,
//! range or curve made only of numbers) that has an op goes through it, so the World
//! never holds a value its own setters would refuse. The file is not rewritten; the
//! next Save writes the held values.
//!
//! Only a field the ops moved is written back, through its own op: an op's other
//! effects (a reverb's `params` making its preset `Custom`) land only when its value
//! was out of bounds, never on a file that was already in range. A non-finite value
//! cannot come from a file (JSON has none), so this holds bounds, not NaN.

use serde_json::{Map, Value};

use super::kinds::KINDS;
use super::table::{Access, Inspect, Values};
use super::value::same;
use crate::ecs::World;

/// Hold every entity's components (a whole scene was applied).
pub fn hold_world(world: &mut World) {
    for id in world.ids().to_vec() {
        hold_entity(world, id);
    }
}

/// Hold each of `id`'s first-class components through its field table.
pub fn hold_entity(world: &mut World, id: u32) {
    for kind in KINDS {
        (kind.hold)(world, id);
    }
}

/// [`hold_entity`] for one component type; nothing without one, or when no field
/// of it has an op.
pub(super) fn hold<C: Inspect>(world: &mut World, id: u32) {
    if C::FIELDS
        .iter()
        .all(|f| matches!(f.access, Access::ReadOnly(_)))
    {
        return;
    }
    C::update(world, id, &mut |live| {
        if let Err(e) = hold_one(live) {
            log::warn!("[Scene] entity {id}: a loaded value could not be held: {e}");
        }
    });
}

/// Run `live`'s numeric fields through their ops on a copy, then write back the
/// ones that moved.
fn hold_one<C: Inspect>(live: &mut C) -> Result<(), String> {
    let loaded = serde_json::to_value(&*live).map_err(|e| e.to_string())?;
    let numeric = |name: &str| loaded.get(name).is_some_and(is_numeric);
    let mut draft = live.clone();
    run(&mut draft, &loaded, &Map::new(), &numeric);
    let held = serde_json::to_value(&draft).map_err(|e| e.to_string())?;
    let moved: Map<String, Value> = C::FIELDS
        .iter()
        .filter(|f| numeric(f.name))
        .filter_map(|f| {
            let value = held.get(f.name)?;
            (!same(value, &loaded[f.name])).then(|| (f.name.to_owned(), value.clone()))
        })
        .collect();
    if !moved.is_empty() {
        run(live, &loaded, &moved, &|name| moved.contains_key(name));
    }
    Ok(())
}

/// Call the op of every field `pick` names, in the table's order, with its value
/// in `changes`, else in `current`; a group's op once, for any of its fields. An op
/// that refuses its value leaves the field as it was.
fn run<C: Inspect>(
    c: &mut C,
    current: &Value,
    changes: &Map<String, Value>,
    pick: &dyn Fn(&str) -> bool,
) {
    let mut groups_run: Vec<&str> = Vec::new();
    for field in C::FIELDS {
        let outcome = match &field.access {
            Access::Op(op) if pick(field.name) => {
                let value = changes.get(field.name).or_else(|| current.get(field.name));
                op(c, value.cloned().unwrap_or(Value::Null))
            }
            Access::Group(group) if group.names.iter().any(|n| pick(n)) => {
                if groups_run.contains(&group.names[0]) {
                    continue;
                }
                groups_run.push(group.names[0]);
                (group.write)(c, &Values { changes, current })
            }
            _ => continue,
        };
        if let Err(e) = outcome {
            log::warn!("[Scene] a loaded '{}' was kept as read: {e}", field.name);
        }
    }
}

/// A number, or a list or table made only of numbers (a vector, a colour, a range,
/// a curve's keys).
pub(super) fn is_numeric(value: &Value) -> bool {
    match value {
        Value::Number(_) => true,
        Value::Array(list) => !list.is_empty() && list.iter().all(is_numeric),
        Value::Object(map) => !map.is_empty() && map.values().all(is_numeric),
        _ => false,
    }
}
