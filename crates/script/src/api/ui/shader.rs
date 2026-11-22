use mlua::{IntoLua, Value};

use super::super::material::shader::params::floats;
use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_edit::authoring::ui::shader as ops;
use rusty_scene::Scene;

pub fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    register_name(lua, table, scene)?;
    register_params(lua, table, scene)
}

fn register_name(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetShader",
        lua.create_function(move |lua, (id, name): (EntityArg, Option<String>)| {
            let id = id.live("UI", "SetShader")?;
            let scene = &*scene.get(lua)?;
            let world = &mut scene.borrow_mut().world;
            match ops::for_each_graphic(world, id, |slot| ops::set_shader(slot, name.clone())) {
                0 => Err(no_graphic(id)),
                _ => Ok(()),
            }
        }),
    )?;
    put(
        table,
        "GetShader",
        lua.create_function(move |lua, id: EntityArg| {
            let slot = ops::graphic_shader(&scene.get(lua)?.borrow().world, id.read()).flatten();
            Ok(slot.map(|s| s.name))
        }),
    )
}

fn register_params(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetShaderParam",
        lua.create_function(move |lua, (id, name, value): (EntityArg, String, Value)| {
            let id = id.live("UI", "SetShaderParam")?;
            let value = floats(value)?;
            let scene = &*scene.get(lua)?;
            let world = &mut scene.borrow_mut().world;
            let mut results = Vec::new();
            let graphics = ops::for_each_graphic(world, id, |slot| {
                if slot.is_some() {
                    let layout = ops::layout(slot);
                    results
                        .push(layout.and_then(|l| ops::set_param(slot, &l, &name, value.clone())));
                }
            });
            if graphics == 0 {
                return Err(no_graphic(id));
            }
            if results.is_empty() {
                // No graphic has a shader: `layout(&None)` gives its "UI.SetShader first" error.
                results.push(ops::layout(&None).map(drop));
            }
            results.into_iter().collect::<Result<(), _>>().map_err(err)
        }),
    )?;
    put(
        table,
        "GetShaderParam",
        lua.create_function(move |lua, (id, name): (EntityArg, String)| {
            let id = id.read();
            let slot = ops::graphic_shader(&scene.get(lua)?.borrow().world, id)
                .ok_or_else(|| no_graphic(id))?;
            let layout = ops::layout(&slot).map_err(err)?;
            let v = ops::param(&slot, &layout, &name).map_err(err)?;
            match v[..] {
                [x] => x.into_lua(lua),
                _ => v.into_lua(lua),
            }
        }),
    )
}

fn no_graphic(id: u32) -> mlua::Error {
    err(format!("entity {id} has no Image, Shape or Text"))
}

fn err(message: String) -> mlua::Error {
    mlua::Error::RuntimeError(message)
}

#[cfg(test)]
#[path = "shader_tests.rs"]
mod tests;
