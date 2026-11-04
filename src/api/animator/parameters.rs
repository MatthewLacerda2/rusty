//! `Animator.SetBool` / `SetFloat` / `SetInt` / `SetTrigger` (#314): the typed graph
//! parameters the animation-graph evaluator (#316) reads to decide transitions.

use std::cell::RefCell;

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::components::AnimatorParameter;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Register the parameter writers onto the `Animator` table.
pub(super) fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    register_set_bool_float(lua, table, scene, console)?;
    register_set_int_trigger(lua, table, scene, console)
}

/// `SetBool` / `SetFloat` — write a typed graph parameter (#314).
fn register_set_bool_float(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    put(
        table,
        "SetBool",
        lua.create_function(move |lua, (id, name, value): (EntityArg, String, bool)| {
            let id = id.live("Animator", "SetBool")?;
            write_parameter(
                &*scene.get(lua)?,
                &*console.get(lua)?,
                id,
                &name,
                AnimatorParameter::Bool(value),
            );
            Ok(())
        }),
    )?;

    put(
        table,
        "SetFloat",
        lua.create_function(move |lua, (id, name, value): (EntityArg, String, f32)| {
            let id = id.live("Animator", "SetFloat")?;
            write_parameter(
                &*scene.get(lua)?,
                &*console.get(lua)?,
                id,
                &name,
                AnimatorParameter::Float(value),
            );
            Ok(())
        }),
    )
}

/// `SetInt` / `SetTrigger` — the remaining parameter setters (#314). `SetTrigger`
/// takes no value: it latches the one-shot true, and the graph evaluator (#316)
/// auto-clears it when it consumes a transition.
fn register_set_int_trigger(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    put(
        table,
        "SetInt",
        lua.create_function(move |lua, (id, name, value): (EntityArg, String, i32)| {
            let id = id.live("Animator", "SetInt")?;
            write_parameter(
                &*scene.get(lua)?,
                &*console.get(lua)?,
                id,
                &name,
                AnimatorParameter::Int(value),
            );
            Ok(())
        }),
    )?;

    put(
        table,
        "SetTrigger",
        lua.create_function(move |lua, (id, name): (EntityArg, String)| {
            let id = id.live("Animator", "SetTrigger")?;
            write_parameter(
                &*scene.get(lua)?,
                &*console.get(lua)?,
                id,
                &name,
                AnimatorParameter::Trigger(true),
            );
            Ok(())
        }),
    )
}

/// Write one typed parameter on the entity's animator. A name already bound to a
/// DIFFERENT type is a warned no-op, never a panic (#314); a missing entity or
/// animator is silently skipped, like every other `Animator.*` binding.
fn write_parameter(
    scene: &RefCell<Scene>,
    console: &RefCell<ConsoleLogs>,
    id: u32,
    name: &str,
    value: AnimatorParameter,
) {
    let mut scene = scene.borrow_mut();
    let failed = scene
        .world
        .animator_mut(id)
        .map(|mut anim| !anim.set_parameter(name, value))
        .unwrap_or(false);
    if failed {
        let entity_name = scene.world.name(id).map(|n| n.clone()).unwrap_or_default();
        console.borrow_mut().warn(format!(
            "Animator parameter '{}' on entity {} already holds a different type; set ignored",
            name, entity_name
        ));
    }
}
