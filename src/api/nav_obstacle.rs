//! src/api/nav_obstacle.rs — `NavMeshObstacle` namespace (#456).
//!
//! Unity's `NavMeshObstacle`: the shape (box or upright capsule), whether it
//! carves the navmesh and how (only once stationary, the move threshold, the time
//! to stationary), plus what the play tick found (`IsCarving`, `GetVelocity`).
//! Every setter routes through the shared `scene::authoring::nav_obstacle` ops the
//! inspector card uses. Getters return a neutral default without an obstacle;
//! setters are then no-ops. Attach one with
//! `Scene.AddComponent(id, "NavMeshObstacle")`.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::components::{NavMeshObstacleComponent as Obstacle, ObstacleShape};
use crate::scene::authoring::nav_obstacle as ops;
use crate::scene::Scene;

type SceneCell<'s> = &'s RefCell<Scene>;
/// A field's reader and its shared-op writer.
type Field<T> = (fn(&Obstacle) -> T, fn(&mut Obstacle, T));

/// Register the `NavMeshObstacle` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_shape(scope, &t, scene)?;
    register_carving(scope, &t, scene)?;
    lua.globals()
        .set("NavMeshObstacle", t)
        .map_err(|e| e.to_string())
}

/// `id`'s obstacle, cloned, or `None` without one.
fn get(scene: SceneCell, id: u32) -> Option<Obstacle> {
    scene.borrow().world.nav_obstacle(id).map(|o| o.clone())
}

/// Apply a shared op to `id`'s obstacle, if it has one.
fn write(scene: SceneCell, id: u32, op: impl FnOnce(&mut Obstacle)) {
    if let Some(mut o) = scene.borrow_mut().world.nav_obstacle_mut(id) {
        op(&mut o);
    }
}

/// `Get<name>` / `Set<name>` over one `f32` field (`0` without an obstacle).
fn f32_pair<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: SceneCell<'scope>,
    name: &str,
    (read, set): Field<f32>,
) -> Reg {
    let f = scope.create_function(move |_, id: u32| Ok(get(scene, id).map_or(0.0, |o| read(&o))));
    put(t, &format!("Get{name}"), f)?;
    let f = scope.create_function(move |_, (id, v): (u32, f32)| {
        write(scene, id, |o| set(o, v));
        Ok(())
    });
    put(t, &format!("Set{name}"), f)
}

/// `Get<name>` / `Set<name>` over one `bool` field (`false` without an obstacle).
fn bool_pair<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: SceneCell<'scope>,
    name: &str,
    (read, set): Field<bool>,
) -> Reg {
    let f = scope.create_function(move |_, id: u32| Ok(get(scene, id).is_some_and(|o| read(&o))));
    put(t, &format!("Get{name}"), f)?;
    let f = scope.create_function(move |_, (id, v): (u32, bool)| {
        write(scene, id, |o| set(o, v));
        Ok(())
    });
    put(t, &format!("Set{name}"), f)
}

/// `Get<name>` / `Set<name>` over one `Vec3` field, as `x, y, z`.
fn vec3_pair<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: SceneCell<'scope>,
    name: &str,
    (read, set): Field<Vec3>,
) -> Reg {
    let f = scope.create_function(move |_, id: u32| {
        let v = get(scene, id).map_or(Vec3::ZERO, |o| read(&o));
        Ok((v.x, v.y, v.z))
    });
    put(t, &format!("Get{name}"), f)?;
    let f = scope.create_function(move |_, (id, x, y, z): (u32, f32, f32, f32)| {
        write(scene, id, |o| set(o, Vec3::new(x, y, z)));
        Ok(())
    });
    put(t, &format!("Set{name}"), f)
}

/// `Shape`, `Center`, `Size`, `Radius`, `Height`.
fn register_shape<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: SceneCell<'scope>,
) -> Reg {
    let f = scope.create_function(move |_, id: u32| Ok(get(scene, id).map(|o| o.shape.as_str())));
    put(t, "GetShape", f)?;
    let f = scope.create_function(move |_, (id, name): (u32, String)| {
        let shape = ObstacleShape::parse(&name).ok_or_else(|| {
            mlua::Error::RuntimeError(format!(
                "NavMeshObstacle.SetShape: unknown shape '{name}' (Box, Capsule)"
            ))
        })?;
        write(scene, id, |o| ops::set_shape(o, shape));
        Ok(())
    });
    put(t, "SetShape", f)?;
    vec3_pair(scope, t, scene, "Center", (|o| o.center, ops::set_center))?;
    vec3_pair(scope, t, scene, "Size", (|o| o.size, ops::set_size))?;
    f32_pair(scope, t, scene, "Radius", (|o| o.radius, ops::set_radius))?;
    f32_pair(scope, t, scene, "Height", (|o| o.height, ops::set_height))
}

/// `Active`, `Carving`, `CarveOnlyStationary`, `MoveThreshold`,
/// `TimeToStationary`, and the runtime `IsCarving` / `GetVelocity`.
fn register_carving<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: SceneCell<'scope>,
) -> Reg {
    bool_pair(scope, t, scene, "Active", (|o| o.active, ops::set_active))?;
    bool_pair(scope, t, scene, "Carving", (|o| o.carve, ops::set_carve))?;
    let only: Field<bool> = (|o| o.carve_only_stationary, ops::set_carve_only_stationary);
    bool_pair(scope, t, scene, "CarveOnlyStationary", only)?;
    let threshold: Field<f32> = (|o| o.move_threshold, ops::set_move_threshold);
    f32_pair(scope, t, scene, "MoveThreshold", threshold)?;
    let wait: Field<f32> = (|o| o.time_to_stationary, ops::set_time_to_stationary);
    f32_pair(scope, t, scene, "TimeToStationary", wait)?;
    let f =
        scope.create_function(move |_, id: u32| Ok(get(scene, id).is_some_and(|o| o.is_carving())));
    put(t, "IsCarving", f)?;
    let f = scope.create_function(move |_, id: u32| {
        let v = get(scene, id).map_or(Vec3::ZERO, |o| o.velocity);
        Ok((v.x, v.y, v.z))
    });
    put(t, "GetVelocity", f)
}

#[cfg(test)]
#[path = "nav_obstacle_tests.rs"]
mod tests;
