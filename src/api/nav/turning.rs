//! src/api/nav/turning.rs — how a `NavMeshAgent` turns to face where it steers
//! (#744): `Get`/`SetUpdateRotation` (Unity's `updateRotation`),
//! `Get`/`SetAngularSpeed` (degrees/second, Unity's `angularSpeed`) and
//! `Get`/`SetAngularAcceleration` (degrees/second²). Getters on an entity without an
//! agent return `false` / `0`, as `GetBaseOffset` does.

use std::cell::RefCell;

use mlua::{FromLua, IntoLua, Table};

use super::super::{put, Reg};
use crate::components::NavMeshAgentComponent;
use crate::scene::authoring::nav_agent as nav_ops;
use crate::scene::Scene;

/// The turning getters and setters on `NavMeshAgent`.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    t: &Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    field(
        scope,
        t,
        scene,
        "UpdateRotation",
        |a| a.update_rotation,
        nav_ops::set_update_rotation,
    )?;
    field(
        scope,
        t,
        scene,
        "AngularSpeed",
        |a| a.angular_speed,
        nav_ops::set_angular_speed,
    )?;
    field(
        scope,
        t,
        scene,
        "AngularAcceleration",
        |a| a.angular_acceleration,
        nav_ops::set_angular_acceleration,
    )
}

/// `Get<name>(id)` reading through `get` and `Set<name>(id, value)` writing through
/// the shared authoring op `set`.
fn field<'scope, T>(
    scope: &'scope mlua::Scope<'scope, '_>,
    t: &Table,
    scene: &'scope RefCell<Scene>,
    name: &str,
    get: fn(&NavMeshAgentComponent) -> T,
    set: fn(&mut NavMeshAgentComponent, T),
) -> Reg
where
    T: Default + IntoLua + FromLua + 'static,
{
    let f = scope.create_function(move |_, id: u32| {
        Ok(scene
            .borrow()
            .world
            .nav_agent(id)
            .map_or_else(T::default, |a| get(&a)))
    });
    put(t, &format!("Get{name}"), f)?;
    let f = scope.create_function(move |_, (id, value): (u32, T)| {
        if let Some(mut a) = scene.borrow_mut().world.nav_agent_mut(id) {
            set(&mut a, value);
        }
        Ok(())
    });
    put(t, &format!("Set{name}"), f)
}
