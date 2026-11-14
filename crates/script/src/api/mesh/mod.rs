//! crates/script/src/api/mesh/ — `Mesh` namespace (#1714, #2059).
//!
//! Meshes made from vertex data and which mesh an entity draws live in `library`
//! (#2059). This file holds the per-renderer settings of an entity's `MeshComponent` that have a verb of
//! their own: the small-object cull's max draw distance (Unreal's *Max Draw
//! Distance*) and min screen size (Unity `LODGroup`'s relative height). Every
//! setter routes through the shared `scene::authoring::rendering::mesh` ops the
//! Inspector's Mesh card uses; `Component.Set(id, "Mesh", { … })` writes the same
//! fields. Getters return `0` (off) without a Mesh; setters are then no-ops. Whether
//! a camera culled the mesh this frame is the renderer's and is not readable here:
//! the sim never depends on it.

use mlua::{Lua, Table};

mod library;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::MeshComponent;
use rusty_scene::authoring::rendering::mesh as ops;
use rusty_scene::Scene;

/// Register the `Mesh` namespace onto `lua`.
pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    let distance: Pair = (|m| m.max_draw_distance, ops::set_max_draw_distance);
    register_pair(lua, &t, scene, "MaxDrawDistance", distance)?;
    let size: Pair = (|m| m.min_screen_size, ops::set_min_screen_size);
    register_pair(lua, &t, scene, "MinScreenSize", size)?;
    library::register(lua, &t, scene)?;
    lua.globals().set("Mesh", t).map_err(|e| e.to_string())
}

/// One `f32` field's read and its authoring op.
type Pair = (fn(&MeshComponent) -> f32, fn(&mut MeshComponent, f32));

/// `Get<name>(id)` and `Set<name>(id, value)` over `(get, set)`.
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
