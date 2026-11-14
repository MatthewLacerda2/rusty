//! crates/script/src/api/nav_obstacle.rs — `NavMeshObstacle` namespace (#456).
//!
//! Unity's `NavMeshObstacle`: the shape (box or upright capsule), whether it
//! carves the navmesh and how (only once stationary, the move threshold, the time
//! to stationary), plus what the play tick found (`IsCarving`, `GetVelocity`).
//! Every setter routes through the shared `scene::authoring::nav::obstacle` ops the
//! inspector card uses. Getters return a neutral default without an obstacle;
//! setters are then no-ops. Attach one with
//! `Scene.AddComponent(id, "NavMeshObstacle")`.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::{NavMeshObstacleComponent as Obstacle, ObstacleShape};
use rusty_scene::authoring::nav::obstacle as ops;
use rusty_scene::Scene;

/// A field's reader and its shared-op writer.
type Field<T> = (fn(&Obstacle) -> T, fn(&mut Obstacle, T));

/// Register the `NavMeshObstacle` namespace onto `lua`.
pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_shape(lua, &t, scene)?;
    register_carving(lua, &t, scene)?;
    lua.globals()
        .set("NavMeshObstacle", t)
        .map_err(|e| e.to_string())
}

/// `id`'s obstacle, cloned, or `None` without one.
fn get(scene: &RefCell<Scene>, id: u32) -> Option<Obstacle> {
    scene.borrow().world.nav_obstacle(id).map(|o| o.clone())
}

/// Apply a shared op to `id`'s obstacle, if it has one.
fn write(scene: &RefCell<Scene>, id: u32, op: impl FnOnce(&mut Obstacle)) {
    if let Some(mut o) = scene.borrow_mut().world.nav_obstacle_mut(id) {
        op(&mut o);
    }
}

/// `Get<name>` / `Set<name>` over one `f32` field (`0` without an obstacle).
fn f32_pair(
    lua: &mlua::Lua,
    t: &Table,
    scene: ApiCell<Scene>,
    name: &str,
    (read, set): Field<f32>,
) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(get(&*scene.get(lua)?, id.read()).map_or(0.0, |o| read(&o)))
    });
    put(t, &format!("Get{name}"), f)?;
    let verb = format!("Set{name}");
    let f = lua.create_function(move |lua, (id, v): (EntityArg, f32)| {
        let id = id.live("NavMeshObstacle", &verb)?;
        write(&*scene.get(lua)?, id, |o| set(o, v));
        Ok(())
    });
    put(t, &format!("Set{name}"), f)
}

/// `Get<name>` / `Set<name>` over one `bool` field (`false` without an obstacle).
fn bool_pair(
    lua: &mlua::Lua,
    t: &Table,
    scene: ApiCell<Scene>,
    name: &str,
    (read, set): Field<bool>,
) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(get(&*scene.get(lua)?, id.read()).is_some_and(|o| read(&o)))
    });
    put(t, &format!("Get{name}"), f)?;
    let verb = format!("Set{name}");
    let f = lua.create_function(move |lua, (id, v): (EntityArg, bool)| {
        let id = id.live("NavMeshObstacle", &verb)?;
        write(&*scene.get(lua)?, id, |o| set(o, v));
        Ok(())
    });
    put(t, &format!("Set{name}"), f)
}

/// `Get<name>` / `Set<name>` over one `Vec3` field, as `x, y, z`.
fn vec3_pair(
    lua: &mlua::Lua,
    t: &Table,
    scene: ApiCell<Scene>,
    name: &str,
    (read, set): Field<Vec3>,
) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        let v = get(&*scene.get(lua)?, id.read()).map_or(Vec3::ZERO, |o| read(&o));
        Ok((v.x, v.y, v.z))
    });
    put(t, &format!("Get{name}"), f)?;
    let verb = format!("Set{name}");
    let f = lua.create_function(move |lua, (id, x, y, z): (EntityArg, f32, f32, f32)| {
        let id = id.live("NavMeshObstacle", &verb)?;
        write(&*scene.get(lua)?, id, |o| set(o, Vec3::new(x, y, z)));
        Ok(())
    });
    put(t, &format!("Set{name}"), f)
}

/// `Shape`, `Center`, `Size`, `Radius`, `Height`.
fn register_shape(lua: &mlua::Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(get(&*scene.get(lua)?, id.read()).map(|o| o.shape.name()))
    });
    put(t, "GetShape", f)?;
    let f = lua.create_function(move |lua, (id, name): (EntityArg, String)| {
        let id = id.live("NavMeshObstacle", "SetShape")?;
        let shape = ObstacleShape::parse(&name).ok_or_else(|| {
            mlua::Error::RuntimeError(format!(
                "NavMeshObstacle.SetShape: unknown shape '{name}' (Box, Capsule)"
            ))
        })?;
        write(&*scene.get(lua)?, id, |o| ops::set_shape(o, shape));
        Ok(())
    });
    put(t, "SetShape", f)?;
    vec3_pair(lua, t, scene, "Center", (|o| o.center, ops::set_center))?;
    vec3_pair(lua, t, scene, "Size", (|o| o.size, ops::set_size))?;
    f32_pair(lua, t, scene, "Radius", (|o| o.radius, ops::set_radius))?;
    f32_pair(lua, t, scene, "Height", (|o| o.height, ops::set_height))
}

/// `Active`, `Carving`, `CarveOnlyStationary`, `MoveThreshold`,
/// `TimeToStationary`, and the runtime `IsCarving` / `GetVelocity`.
fn register_carving(lua: &mlua::Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    bool_pair(lua, t, scene, "Active", (|o| o.active, ops::set_active))?;
    bool_pair(lua, t, scene, "Carving", (|o| o.carve, ops::set_carve))?;
    let only: Field<bool> = (|o| o.carve_only_stationary, ops::set_carve_only_stationary);
    bool_pair(lua, t, scene, "CarveOnlyStationary", only)?;
    let threshold: Field<f32> = (|o| o.move_threshold, ops::set_move_threshold);
    f32_pair(lua, t, scene, "MoveThreshold", threshold)?;
    let wait: Field<f32> = (|o| o.time_to_stationary, ops::set_time_to_stationary);
    f32_pair(lua, t, scene, "TimeToStationary", wait)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(get(&*scene.get(lua)?, id.read()).is_some_and(|o| o.is_carving()))
    });
    put(t, "IsCarving", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        let v = get(&*scene.get(lua)?, id.read()).map_or(Vec3::ZERO, |o| o.velocity);
        Ok((v.x, v.y, v.z))
    });
    put(t, "GetVelocity", f)
}

#[cfg(test)]
#[path = "nav_obstacle_tests.rs"]
mod tests;
