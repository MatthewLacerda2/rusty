//! Angular speed is degrees/second, angular acceleration degrees/second². Getters without an agent
//! return `false` / `0`.

use mlua::{FromLua, IntoLua, Table};

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::NavMeshAgentComponent;
use rusty_edit::authoring::nav::agent as nav_ops;
use rusty_scene::Scene;

pub(super) fn register(lua: &mlua::Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    field(
        lua,
        t,
        scene,
        "UpdateRotation",
        |a| a.update_rotation,
        nav_ops::set_update_rotation,
    )?;
    field(
        lua,
        t,
        scene,
        "AngularSpeed",
        |a| a.angular_speed,
        nav_ops::set_angular_speed,
    )?;
    field(
        lua,
        t,
        scene,
        "AngularAcceleration",
        |a| a.angular_acceleration,
        nav_ops::set_angular_acceleration,
    )
}

fn field<T>(
    lua: &mlua::Lua,
    t: &Table,
    scene: ApiCell<Scene>,
    name: &str,
    get: fn(&NavMeshAgentComponent) -> T,
    set: fn(&mut NavMeshAgentComponent, T),
) -> Reg
where
    T: Default + IntoLua + FromLua + 'static,
{
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(scene
            .get(lua)?
            .borrow()
            .world
            .nav_agent(id.read())
            .map_or_else(T::default, |a| get(&a)))
    });
    put(t, &format!("Get{name}"), f)?;
    let verb = format!("Set{name}");
    let f = lua.create_function(move |lua, (id, value): (EntityArg, T)| {
        let id = id.live("NavMeshAgent", &verb)?;
        if let Some(mut a) = scene.get(lua)?.borrow_mut().world.nav_agent_mut(id) {
            set(&mut a, value);
        }
        Ok(())
    });
    put(t, &format!("Set{name}"), f)
}
