use serde_json::{Map, Value};

use super::kinds::KINDS;
use super::table::{Access, Field, Inspect};
use super::value::same;
use super::write::{named, run_ops, serialized};
use rusty_components::Entity;
use rusty_ecs::World;

pub fn hold_world(world: &mut World) {
    for id in world.ids().to_vec() {
        hold_entity(world, id);
    }
}

pub fn hold_entity(world: &mut World, id: u32) {
    for kind in KINDS {
        (kind.hold)(world, id, kind.name);
    }
}

pub fn hold_entities(entities: &mut [Entity]) {
    let mut world = World::new();
    for entity in entities.iter() {
        world.insert_entity(entity.clone());
    }
    for entity in entities.iter_mut() {
        hold_entity(&mut world, entity.id);
        if let Some(mut held) = world.entity_document(entity.id) {
            held.parent_bone = std::mem::take(&mut entity.parent_bone);
            held.bone_refs = std::mem::take(&mut entity.bone_refs);
            *entity = held;
        }
    }
}

pub(super) fn hold<C: Inspect>(world: &mut World, id: u32, kind: &str) {
    if !C::FIELDS.iter().any(held_on_load) {
        return;
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
    let mut draft = C::draft(live);
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

fn held_on_load<C>(field: &Field<C>) -> bool {
    field.unheld.is_none() && !matches!(field.access, Access::ReadOnly(_))
}

#[cfg(test)]
pub(super) fn named_held<C: Inspect>(name: &str) -> bool {
    C::FIELDS.iter().any(|f| f.name == name && held_on_load(f))
}

pub(super) fn has_number(value: &Value) -> bool {
    match value {
        Value::Number(_) => true,
        Value::Array(list) => list.iter().any(has_number),
        Value::Object(map) => map.values().any(has_number),
        _ => false,
    }
}

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
