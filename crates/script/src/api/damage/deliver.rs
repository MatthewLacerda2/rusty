use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table, Value};

use crate::scripting::callbacks::ON_TAKE_DAMAGE;
use crate::scripting::instances;
use rusty_ai::perception;
use rusty_scene::Scene;
use rusty_ui::events::nearest;

fn handles(lua: &Lua, id: u32) -> bool {
    instances::handlers(lua, id, ON_TAKE_DAMAGE).is_ok_and(|h| !h.is_empty())
}

pub(super) fn receiver(lua: &Lua, scene: &Scene, id: u32) -> Option<u32> {
    let world = &scene.world;
    nearest(world, id, |c| {
        world.is_active_in_hierarchy(c) && handles(lua, c)
    })
}

pub(super) fn to_receiver(
    lua: &Lua,
    scene: &RefCell<Scene>,
    id: u32,
    amount: f64,
    event: &Table,
) -> mlua::Result<f64> {
    sense_event(scene, id, amount, event)?;
    // Bound apart so the scene borrow ends before a handler runs; a `match` scrutinee's would not.
    let found = receiver(lua, &scene.borrow(), id);
    match found {
        Some(r) => call(lua, r, amount, event),
        None => Ok(0.0),
    }
}

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

pub(super) fn sense(
    scene: &RefCell<Scene>,
    struck: u32,
    instigator: Option<u32>,
    at: Vec3,
    hit: (f64, &str),
) {
    perception::report_damage(&mut scene.borrow_mut(), struck, instigator, at, hit);
}
