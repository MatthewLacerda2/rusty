use std::cell::RefCell;

use super::super::{put, Reg};
use crate::api::ApiCell;
use rusty_navigation::{NavBounds, NavMeshSettings, NavigationGraph};
use rusty_scene::Scene;

pub(super) fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    nav: ApiCell<NavigationGraph>,
) -> Reg {
    register_settings_getters(lua, table, scene)?;
    register_settings_setters(lua, table, scene, nav)?;
    register_bounds(lua, table, scene, nav)
}

fn register_settings_getters(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetAgentRadius",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().nav_settings.agent_radius)),
    )?;
    put(
        table,
        "GetAgentHeight",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().nav_settings.agent_height)),
    )?;
    put(
        table,
        "GetMaxSlope",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().nav_settings.max_slope)),
    )?;
    put(
        table,
        "GetMaxStep",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().nav_settings.max_step)),
    )?;
    put(
        table,
        "GetGridSpacing",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().nav_settings.grid_spacing)),
    )
}

/// The setters write `scene.nav_settings`, then re-bake the shared graph so the knob is consumed (the bake is the read-site).
fn register_settings_setters(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    nav: ApiCell<NavigationGraph>,
) -> Reg {
    put(
        table,
        "SetAgentRadius",
        lua.create_function(move |lua, r: f32| {
            set_and_rebake(&*scene.get(lua)?, &*nav.get(lua)?, |s| s.agent_radius = r)
        }),
    )?;
    put(
        table,
        "SetAgentHeight",
        lua.create_function(move |lua, h: f32| {
            set_and_rebake(&*scene.get(lua)?, &*nav.get(lua)?, |s| s.agent_height = h)
        }),
    )?;
    put(
        table,
        "SetMaxSlope",
        lua.create_function(move |lua, v: f32| {
            set_and_rebake(&*scene.get(lua)?, &*nav.get(lua)?, |s| s.max_slope = v)
        }),
    )?;
    put(
        table,
        "SetMaxStep",
        lua.create_function(move |lua, v: f32| {
            set_and_rebake(&*scene.get(lua)?, &*nav.get(lua)?, |s| s.max_step = v)
        }),
    )?;
    put(
        table,
        "SetGridSpacing",
        lua.create_function(move |lua, v: f32| {
            set_and_rebake(&*scene.get(lua)?, &*nav.get(lua)?, |s| s.grid_spacing = v)
        }),
    )
}

fn register_bounds(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    nav: ApiCell<NavigationGraph>,
) -> Reg {
    put(
        table,
        "GetBounds",
        lua.create_function(move |lua, ()| {
            let b = nav.get(lua)?.borrow().bounds();
            let authored = scene.get(lua)?.borrow().nav_settings.bounds.is_some();
            Ok((b.min_x, b.max_x, b.min_z, b.max_z, authored))
        }),
    )?;
    put(
        table,
        "SetBounds",
        lua.create_function(
            move |lua, (min_x, max_x, min_z, max_z): (f32, f32, f32, f32)| {
                let b = NavBounds {
                    min_x,
                    max_x,
                    min_z,
                    max_z,
                };
                if !b.is_valid() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Navigation.SetBounds: need finite bounds with min < max, got \
                         x [{min_x}, {max_x}] z [{min_z}, {max_z}]"
                    )));
                }
                set_and_rebake(&*scene.get(lua)?, &*nav.get(lua)?, |s| s.bounds = Some(b))
            },
        ),
    )?;
    put(
        table,
        "ClearBounds",
        lua.create_function(move |lua, ()| {
            set_and_rebake(&*scene.get(lua)?, &*nav.get(lua)?, |s| s.bounds = None)
        }),
    )
}

/// The scene's mutable borrow is scoped to the write alone, so the bake's immutable borrow never overlaps it.
pub(super) fn set_and_rebake(
    scene: &RefCell<Scene>,
    nav: &RefCell<NavigationGraph>,
    apply: impl FnOnce(&mut NavMeshSettings),
) -> mlua::Result<()> {
    apply(&mut scene.borrow_mut().nav_settings);
    nav.borrow_mut().bake(&scene.borrow());
    Ok(())
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod settings_tests;
