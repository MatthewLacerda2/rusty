//! src/scene/authoring/fields/hold.rs — a loaded component's numbers through their
//! ops, once (#1418).
//!
//! The authoring ops hold every bound a field has (`sanitize`, #1361), but only on
//! a write. A scene or prefab file reaches the World without one, so a hand-edited
//! `volume: 4` would play at 4×. [`hold_world`] closes that at the source: when a
//! scene file is applied (and [`hold_entities`] when a prefab file is read), every
//! field that holds a number (a number, a vector, or a struct or list with one
//! inside: SSAO's `intensity`, a collider material's `friction`) goes through its
//! op, so the World never holds a value its own setters would refuse. A field the
//! table marks `unheld!` is left as read, with its reason. The file is not
//! rewritten; the next Save writes the held values.
//!
//! Only a field whose numbers the ops moved is written back, through its own op:
//! an op's other effects (a reverb's `params` making its preset `Custom`) land only
//! when a number was out of bounds, so a file already in range loads exactly as
//! saved. A non-finite value cannot come from a file (JSON has none), so this holds
//! bounds, not NaN.

use serde_json::{Map, Value};

use super::kinds::KINDS;
use super::table::{Access, Field, Inspect};
use super::value::same;
use super::write::{named, run_ops, serialized};
use crate::components::Entity;
use crate::ecs::World;

/// Hold every entity's components (a scene file was applied).
pub fn hold_world(world: &mut World) {
    for id in world.ids().to_vec() {
        hold_entity(world, id);
    }
}

/// Hold each of `id`'s first-class components through its field table.
pub fn hold_entity(world: &mut World, id: u32) {
    for kind in KINDS {
        (kind.hold)(world, id, kind.name);
    }
}

/// Hold documents read from a file (a prefab's entities), through a scratch World
/// the ops can reach them in.
pub fn hold_entities(entities: &mut [Entity]) {
    let mut world = World::new();
    for entity in entities.iter() {
        world.insert_entity(entity.clone());
    }
    for entity in entities.iter_mut() {
        hold_entity(&mut world, entity.id);
        if let Some(mut held) = world.entity_document(entity.id) {
            // Document-only: the World keeps no column for them.
            held.parent_bone = std::mem::take(&mut entity.parent_bone);
            held.bone_refs = std::mem::take(&mut entity.bone_refs);
            *entity = held;
        }
    }
}

/// [`hold_entity`] for one component type, named `kind` in its warnings; nothing
/// without one.
pub(super) fn hold<C: Inspect>(world: &mut World, id: u32, kind: &str) {
    if !C::FIELDS.iter().any(held_on_load) {
        return; // Transform: nothing to hold, so no copy to make
    }
    C::update(world, id, &mut |live| {
        let mut refused = |field: &str, e: String| {
            log::warn!(
                "[Scene] entity {id}: a loaded {kind} value was kept as read: {}",
                named(field, e)
            );
            Ok(())
        };
        if let Err(e) = hold_one(live, &mut refused) {
            log::warn!("[Scene] entity {id}: a loaded {kind} could not be held: {e}");
        }
    });
}

/// Run `live`'s held fields through their ops on a copy, then write back the ones
/// whose numbers moved.
fn hold_one<C: Inspect>(
    live: &mut C,
    refused: &mut dyn FnMut(&str, String) -> Result<(), String>,
) -> Result<(), String> {
    let loaded = serialized(live)?;
    let held: Map<String, Value> = C::FIELDS
        .iter()
        .filter(|f| held_on_load(f))
        .filter_map(|f| Some((f.name.to_owned(), loaded.get(f.name)?.clone())))
        .filter(|(_, v)| has_number(v))
        .collect();
    if held.is_empty() {
        return Ok(());
    }
    let mut draft = live.clone();
    run_ops(&mut draft, &held, refused)?;
    let after = serialized(&draft)?;
    let moved: Map<String, Value> = held
        .iter()
        .filter_map(|(name, was)| {
            let now = after.get(name)?;
            numbers_moved(now, was).then(|| (name.clone(), now.clone()))
        })
        .collect();
    if moved.is_empty() {
        return Ok(());
    }
    run_ops(live, &moved, refused)
}

/// Whether a load runs `field`'s op: it has one, and the table does not mark it
/// `unheld!`.
fn held_on_load<C>(field: &Field<C>) -> bool {
    field.unheld.is_none() && !matches!(field.access, Access::ReadOnly(_))
}

/// Whether a load runs `C`'s field `name` (the hold tests).
#[cfg(test)]
pub(super) fn named_held<C: Inspect>(name: &str) -> bool {
    C::FIELDS.iter().any(|f| f.name == name && held_on_load(f))
}

/// Whether `value` is or holds a number.
pub(super) fn has_number(value: &Value) -> bool {
    match value {
        Value::Number(_) => true,
        Value::Array(list) => list.iter().any(has_number),
        Value::Object(map) => map.values().any(has_number),
        _ => false,
    }
}

/// Whether `now` differs from `was` in a number (or in the shape that holds
/// them); a change only to a name or a flag beside the numbers does not count.
fn numbers_moved(now: &Value, was: &Value) -> bool {
    match (now, was) {
        (Value::Array(a), Value::Array(b)) => {
            a.len() != b.len() || a.iter().zip(b).any(|(a, b)| numbers_moved(a, b))
        }
        (Value::Object(a), Value::Object(b)) => a.keys().chain(b.keys()).any(|k| {
            let null = Value::Null;
            numbers_moved(a.get(k).unwrap_or(&null), b.get(k).unwrap_or(&null))
        }),
        (a, b) => (has_number(a) || has_number(b)) && !same(a, b),
    }
}
