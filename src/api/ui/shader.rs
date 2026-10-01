//! src/api/ui/shader.rs — `UI.SetShader` / `GetShader` / `SetShaderParam` /
//! `GetShaderParam` (#427): a graphic's custom ui shader, Unity's `Graphic.material`.
//!
//! Thin adapters over `authoring::ui_shader`. An entity's graphics are its `Image`,
//! `Shape` and `Text`: `SetShader` names the shader on every one it has, and the param
//! verbs act on them the same way, checked against the shader's baked layout (a
//! param it does not expose at runtime, or a wrong number of values, raises an error
//! listing the runtime params there are). Values are numbers or arrays of numbers,
//! exactly as `Material.SetShaderParam` takes them (#399).

use std::cell::RefCell;

use mlua::{IntoLua, Value};

use super::super::material::shader_params::floats;
use super::super::{put, Reg};
use crate::scene::authoring::ui_shader as ops;
use crate::scene::Scene;

/// Register the four verbs onto the `UI` `table`.
pub fn register<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    register_name(scope, table, scene)?;
    register_params(scope, table, scene)
}

/// `SetShader` / `GetShader`.
fn register_name<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetShader",
        scope.create_function(|_, (id, name): (u32, Option<String>)| {
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
        scope.create_function(|_, id: u32| {
            let slot = ops::graphic_shader(&scene.borrow().world, id).flatten();
            Ok(slot.map(|s| s.name))
        }),
    )
}

/// `SetShaderParam` / `GetShaderParam`.
fn register_params<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetShaderParam",
        scope.create_function(|_, (id, name, value): (u32, String, Value)| {
            let value = floats(value)?;
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
                // Shaded by none of its graphics: the "name one first" error.
                results.push(ops::layout(&None).map(drop));
            }
            results.into_iter().collect::<Result<(), _>>().map_err(err)
        }),
    )?;
    put(
        table,
        "GetShaderParam",
        scope.create_function(|lua, (id, name): (u32, String)| {
            let slot =
                ops::graphic_shader(&scene.borrow().world, id).ok_or_else(|| no_graphic(id))?;
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
