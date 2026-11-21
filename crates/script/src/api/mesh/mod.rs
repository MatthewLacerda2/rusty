//! Max draw distance matches Unreal's *Max Draw Distance*; min screen size matches Unity `LODGroup`'s relative height.
//! Getters return `0` (off) without a Mesh. Whether a camera culled the mesh this frame is the renderer's and not readable: the sim never depends on it.

use mlua::{Lua, Table};

mod library;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::MeshComponent;
use rusty_edit::authoring::rendering::mesh as ops;
use rusty_scene::Scene;

pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    let distance: Pair = (|m| m.max_draw_distance, ops::set_max_draw_distance);
    register_pair(lua, &t, scene, "MaxDrawDistance", distance)?;
    let size: Pair = (|m| m.min_screen_size, ops::set_min_screen_size);
    register_pair(lua, &t, scene, "MinScreenSize", size)?;
    library::register(lua, &t, scene)?;
    lua.globals().set("Mesh", t).map_err(|e| e.to_string())
}

type Pair = (fn(&MeshComponent) -> f32, fn(&mut MeshComponent, f32));

fn register_pair(lua: &Lua, t: &Table, scene: ApiCell<Scene>, name: &str, (get, set): Pair) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        let cell = scene.get(lua)?;
        let s = cell.borrow();
        Ok(s.world.mesh(id.read()).map_or(0.0, |m| get(&m)))
    });
    put(t, &format!("Get{name}"), f)?;
    let verb = format!("Set{name}");
    let f = lua.create_function(move |lua, (id, value): (EntityArg, f32)| {
        let id = id.live("Mesh", &verb)?;
        let cell = scene.get(lua)?;
        if let Some(mut mesh) = cell.borrow_mut().world.mesh_mut(id) {
            set(&mut mesh, value);
        }
        Ok(())
    });
    put(t, &format!("Set{name}"), f)
}
