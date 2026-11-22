mod agent;
mod areas;
mod offmesh;
mod path_state;
mod query;
mod settings;
mod turning;

use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::api::ApiCell;
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;

pub fn register(lua: &Lua, scene: ApiCell<Scene>, nav: ApiCell<NavigationGraph>) -> Reg {
    register_navigation(lua, scene, nav)?;
    agent::register(lua, scene, nav)
}

fn register_navigation(lua: &Lua, scene: ApiCell<Scene>, nav: ApiCell<NavigationGraph>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "GetNextPathStep",
        lua.create_function(
            move |lua, (cx, cy, cz, tx, ty, tz): (f32, f32, f32, f32, f32, f32)| {
                let nav = &*nav.get(lua)?;
                let step = nav
                    .borrow()
                    .get_next_path_step(Vec3::new(cx, cy, cz), Vec3::new(tx, ty, tz));
                Ok((step.x, step.y, step.z))
            },
        ),
    )?;

    // A dev build replaces this with one that also saves `<scene>.navmesh`
    // (`dev::lua_surface::bake`).
    put(
        &table,
        "Bake",
        lua.create_function(move |lua, ()| {
            let (scene, nav) = (&*scene.get(lua)?, &*nav.get(lua)?);
            nav.borrow_mut().bake(&scene.borrow());
            Ok(false)
        }),
    )?;
    settings::register(lua, &table, scene, nav)?;
    query::register(lua, &table, nav)?;
    offmesh::register_navigation(lua, &table, scene, nav)?;
    areas::register_navigation(lua, &table, scene, nav)?;

    lua.globals()
        .set("Navigation", table)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod areas_tests;
#[cfg(test)]
mod query_tests;
