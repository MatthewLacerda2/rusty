use std::cell::RefCell;

use glam::Vec3;
use mlua::Table;

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_physics::PhysicsWorld;
use rusty_scene::Scene;
use rusty_time::FIXED_DELTA_TIME;

type AtPoint = (EntityArg, f32, f32, f32, f32, f32, f32);

pub(super) fn register(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> Reg {
    for (name, scale) in [
        ("AddForceAtPosition", FIXED_DELTA_TIME),
        ("AddImpulseAtPosition", 1.0),
    ] {
        let f = lua.create_function(move |lua, (id, x, y, z, px, py, pz): AtPoint| {
            let id = id.live("Physics", name)?;
            let impulse = Vec3::new(x, y, z) * scale;
            apply(
                &*scene.get(lua)?,
                &*physics.get(lua)?,
                id,
                impulse,
                Vec3::new(px, py, pz),
            );
            Ok(())
        });
        put(table, name, f)?;
    }
    Ok(())
}

pub(in crate::api) fn apply(
    scene: &RefCell<Scene>,
    physics: &RefCell<Option<PhysicsWorld>>,
    id: u32,
    impulse: Vec3,
    point: Vec3,
) {
    let response = physics
        .borrow()
        .as_ref()
        .and_then(|p| p.impulse_response(id, impulse, point));
    let mut scene = scene.borrow_mut();
    let Some(mut rb) = scene.world.rigidbody_mut(id) else {
        return;
    };
    if rb.is_kinematic {
        return;
    }
    let (dv, dw) = response.unwrap_or((impulse / rb.mass.max(0.0001), Vec3::ZERO));
    rb.velocity += dv;
    rb.angular_velocity += dw;
}
