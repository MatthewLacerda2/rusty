//! src/api/physics/through.rs — `Physics.RaycastThrough` (#830): every solid a
//! ray crosses as an enter/exit pair, the wallbang query. The penetration
//! counterpart to `RaycastAll`, with its hit shape and layer-mask handling.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Table;

use super::super::{put, Reg};
use super::cast::{accepts, set_owner, set_surface};
use crate::physics::{PhysicsWorld, RayCrossing};
use crate::scene::Scene;

/// One crossing: `{id, bone, bone_name, root, thickness, enter = {distance,
/// point, normal}, exit = {distance, point, normal} | nil}`.
fn crossing_table(lua: &mlua::Lua, scene: &Scene, c: &RayCrossing) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    set_owner(&t, scene, c.enter.id)?;
    t.set("thickness", c.thickness)?;
    let enter = lua.create_table()?;
    set_surface(lua, &enter, &c.enter)?;
    t.set("enter", enter)?;
    if let Some(hit) = &c.exit {
        let exit = lua.create_table()?;
        set_surface(lua, &exit, hit)?;
        t.set("exit", exit)?;
    }
    Ok(t)
}

/// `RaycastThrough(ox,oy,oz, dx,dy,dz, max_distance [, layer_mask])` — an array
/// of crossings sorted by entry distance (ties by entity id). Empty with no
/// live physics world.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    type Args = (f32, f32, f32, f32, f32, f32, f32, Option<u32>);
    put(
        table,
        "RaycastThrough",
        scope.create_function(|lua, (ox, oy, oz, dx, dy, dz, max, mask): Args| {
            let physics = physics.borrow();
            let scene = scene.borrow();
            let crossings = physics.as_ref().map_or_else(Vec::new, |world| {
                world.raycast_through(Vec3::new(ox, oy, oz), Vec3::new(dx, dy, dz), max, |id| {
                    accepts(&scene, id, None, mask)
                })
            });
            let out = lua.create_table()?;
            for (i, c) in crossings.iter().enumerate() {
                out.set(i + 1, crossing_table(lua, &scene, c)?)?;
            }
            Ok(out)
        }),
    )
}
