use glam::Vec3;
use mlua::Table;

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_physics::PhysicsWorld;
use rusty_scene::Scene;

pub(super) fn register(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> Reg {
    register_closest(lua, table, physics)?;
    register_contains(lua, table, physics)?;
    register_bounds(lua, table, scene)
}

fn register_closest(lua: &mlua::Lua, table: &Table, physics: ApiCell<Option<PhysicsWorld>>) -> Reg {
    put(
        table,
        "ClosestPoint",
        lua.create_function(move |lua, (id, x, y, z): (EntityArg, f32, f32, f32)| {
            let physics = &*physics.get(lua)?;
            let physics = physics.borrow();
            let closest = physics
                .as_ref()
                .and_then(|world| world.closest_point_on(id.read(), Vec3::new(x, y, z)));
            Ok(match closest {
                Some(p) => (true, p.x, p.y, p.z),
                None => (false, x, y, z),
            })
        }),
    )
}

fn register_contains(
    lua: &mlua::Lua,
    table: &Table,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> Reg {
    put(
        table,
        "ContainsPoint",
        lua.create_function(move |lua, (id, x, y, z): (EntityArg, f32, f32, f32)| {
            let physics = &*physics.get(lua)?;
            let physics = physics.borrow();
            Ok(physics
                .as_ref()
                .and_then(|world| world.collider_contains_point(id.read(), Vec3::new(x, y, z)))
                .unwrap_or(false))
        }),
    )
}

fn register_bounds(lua: &mlua::Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetBounds",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let bounds = scene.world.collider_bounds(id.read());
            Ok(match bounds {
                Some((min, max)) => (true, min.x, min.y, min.z, max.x, max.y, max.z),
                None => (false, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            })
        }),
    )
}
