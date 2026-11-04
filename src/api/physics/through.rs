//! src/api/physics/through.rs — `Physics.RaycastThrough` (#830): every solid a
//! ray crosses as an enter/exit pair, the wallbang query. The penetration
//! counterpart to `RaycastAll`, with its hit shape and layer-mask handling.

use glam::Vec3;
use mlua::Table;

use super::super::{put, Reg};
use super::cast::{reach, set_owner, set_surface, Filter};
use crate::api::ApiCell;
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

/// `RaycastThrough(ox,oy,oz, dx,dy,dz, max_distance [, layer_mask [,
/// query_triggers]])` — an array
/// of crossings sorted by entry distance (ties by entity id). Empty with no
/// live physics world.
pub(super) fn register(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> Reg {
    type Args = (f32, f32, f32, f32, f32, f32, f32, Option<u32>, Option<bool>);
    put(
        table,
        "RaycastThrough",
        lua.create_function(move |lua, args: Args| {
            let (ox, oy, oz, dx, dy, dz, max, mask, triggers) = args;
            let filter = Filter::new(None, mask, triggers);
            let scene = &*scene.get(lua)?;
            let physics = &*physics.get(lua)?;
            let physics = physics.borrow();
            let scene = scene.borrow();
            let crossings = match (physics.as_ref(), reach(Some(max))) {
                (Some(world), Some(max)) => {
                    let (o, d) = (Vec3::new(ox, oy, oz), Vec3::new(dx, dy, dz));
                    world.raycast_through(o, d, max, |id| filter.accepts(&scene, id))
                }
                _ => Vec::new(),
            };
            let out = lua.create_table()?;
            for (i, c) in crossings.iter().enumerate() {
                out.set(i + 1, crossing_table(lua, &scene, c)?)?;
            }
            Ok(out)
        }),
    )
}
