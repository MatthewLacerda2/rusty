//! crates/script/src/api/damage/deliver.rs — who receives a damage event, and calling them.
//!
//! The receiver is the nearest active ancestor-or-self of the struck entity with a
//! live script defining `OnTakeDamage` (`ui::events::nearest`, the walk the UI
//! pointer callbacks bubble along). Each of its scripts that defines the callback
//! is called in slot order; a handler's number return is what it took, and the
//! next handler is called with that amount (an armour script before a health one).
//!
//! Every hit is also the damage sense's stimulus (#1900, [`sense`]): the struck
//! entity's perceiver (`ai::perception::report_damage`) senses it whether or not
//! anything receives the damage, so a bot with no health script still turns to
//! the shot.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table, Value};

use crate::scripting::callbacks::ON_TAKE_DAMAGE;
use crate::scripting::instances;
use rusty_ai::perception;
use rusty_scene::Scene;
use rusty_ui::events::nearest;

/// Whether `id` has a live script defining `OnTakeDamage`.
fn handles(lua: &Lua, id: u32) -> bool {
    instances::handlers(lua, id, ON_TAKE_DAMAGE).is_ok_and(|h| !h.is_empty())
}

/// The entity a hit on `id` is delivered to, or `None` when nothing up its
/// hierarchy handles damage.
pub(super) fn receiver(lua: &Lua, scene: &Scene, id: u32) -> Option<u32> {
    let world = &scene.world;
    nearest(world, id, |c| {
        world.is_active_in_hierarchy(c) && handles(lua, c)
    })
}

/// Deliver `amount` to the receiver of a hit on `id` ([`receiver`]); what it
/// took, `0` when nothing received it.
pub(super) fn to_receiver(
    lua: &Lua,
    scene: &RefCell<Scene>,
    id: u32,
    amount: f64,
    event: &Table,
) -> mlua::Result<f64> {
    sense_event(scene, id, amount, event)?;
    let found = receiver(lua, &scene.borrow(), id);
    match found {
        Some(r) => call(lua, r, amount, event),
        None => Ok(0.0),
    }
}

/// Call every `OnTakeDamage` on `receiver`, in slot order, chaining the amount
/// each returns; the last amount is what it took. A handler's error propagates.
pub(super) fn call(lua: &Lua, receiver: u32, amount: f64, event: &Table) -> mlua::Result<f64> {
    let mut taken = amount;
    for table in instances::handlers(lua, receiver, ON_TAKE_DAMAGE)? {
        let f: mlua::Function = table.get(ON_TAKE_DAMAGE)?;
        taken = match f.call::<Value>((receiver, taken, event.clone()))? {
            Value::Nil => taken,
            Value::Integer(n) => n as f64,
            Value::Number(n) => n,
            other => {
                return Err(mlua::Error::RuntimeError(format!(
                    "OnTakeDamage on entity {receiver} returned a {}, not a number or nil",
                    other.type_name()
                )))
            }
        };
    }
    Ok(taken)
}

/// The damage sense for a hit on `struck` described by `event`: placed at the
/// instigator, else the hit's point, else the struck entity.
fn sense_event(
    scene: &RefCell<Scene>,
    struck: u32,
    amount: f64,
    event: &Table,
) -> mlua::Result<()> {
    let point = match event.get::<Option<Table>>("hit")? {
        Some(hit) => hit.get::<Option<Table>>("point")?,
        None => None,
    };
    let at = match point {
        Some(p) => Vec3::new(p.get("x")?, p.get("y")?, p.get("z")?),
        None => scene.borrow().world.world_position(struck),
    };
    let instigator: Option<u32> = event.get("instigator")?;
    let kind: String = event.get("type")?;
    sense(scene, struck, instigator, at, (amount, &kind));
    Ok(())
}

/// Report a hit on `struck` to its perceiver, if it has one (`report_damage`).
pub(super) fn sense(
    scene: &RefCell<Scene>,
    struck: u32,
    instigator: Option<u32>,
    at: Vec3,
    hit: (f64, &str),
) {
    perception::report_damage(&mut scene.borrow_mut(), struck, instigator, at, hit);
}
