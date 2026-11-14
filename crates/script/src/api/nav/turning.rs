//! crates/script/src/api/nav/turning.rs — how a `NavMeshAgent` turns to face where it steers
//! (#744): `Get`/`SetUpdateRotation` (Unity's `updateRotation`),
//! `Get`/`SetAngularSpeed` (degrees/second, Unity's `angularSpeed`) and
//! `Get`/`SetAngularAcceleration` (degrees/second²). Getters on an entity without an
//! agent return `false` / `0`, as `GetBaseOffset` does.

use mlua::{FromLua, IntoLua, Table};

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::NavMeshAgentComponent;
use rusty_scene::authoring::nav::agent as nav_ops;
use rusty_scene::Scene;

/// The turning getters and setters on `NavMeshAgent`.
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

/// `Get<name>(id)` reading through `get` and `Set<name>(id, value)` writing through
/// the shared authoring op `set`.
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
