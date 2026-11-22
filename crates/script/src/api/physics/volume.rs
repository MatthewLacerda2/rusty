use glam::{Quat, Vec3};
use mlua::{Function, Table};

use super::super::{put, Reg};
use super::cast::{check, Filter};
use crate::api::ApiCell;
use rusty_physics::PhysicsWorld;
use rusty_scene::Scene;

#[derive(Clone, Copy)]
enum Volume {
    Sphere(Vec3, f32),
    Box(Vec3, Vec3, Quat),
    Capsule(Vec3, Vec3, f32),
}

type Query = (Volume, Option<u32>, Option<bool>);

type Cells = (ApiCell<Scene>, ApiCell<Option<PhysicsWorld>>);

fn ids_in(
    lua: &mlua::Lua,
    (scene, physics): Cells,
    volume: Volume,
    mask: Option<u32>,
    triggers: Option<bool>,
) -> mlua::Result<Vec<u32>> {
    let filter = Filter::new(None, mask, triggers);
    let physics = &*physics.get(lua)?;
    let physics = physics.borrow();
    let Some(world) = physics.as_ref() else {
        return Ok(Vec::new());
    };
    let scene = &*scene.get(lua)?;
    let scene = scene.borrow();
    let accept = |id| filter.accepts(&scene, id);
    Ok(match volume {
        Volume::Sphere(center, radius) => world.overlap_sphere(center, radius, accept),
        Volume::Box(center, half, rotation) => world.overlap_box(center, half, rotation, accept),
        Volume::Capsule(a, b, radius) => world.overlap_capsule(a, b, radius, accept),
    })
}

fn register_pair<A: mlua::FromLuaMulti + 'static>(
    lua: &mlua::Lua,
    table: &Table,
    cells: Cells,
    name: &str,
    volume: fn(&str, A) -> mlua::Result<Query>,
) -> Reg {
    let verb = |prefix: &str| format!("{prefix}{name}");
    let (overlap, check) = (verb("Overlap"), verb("Check"));
    let query = |verb: String, as_bool: bool| -> mlua::Result<Function> {
        lua.create_function(move |lua, args: A| {
            let (volume, mask, triggers) = volume(&verb, args)?;
            let ids = ids_in(lua, cells, volume, mask, triggers)?;
            if as_bool {
                return Ok(mlua::Value::Boolean(!ids.is_empty()));
            }
            lua.create_sequence_from(ids).map(mlua::Value::Table)
        })
    };
    put(table, &overlap, query(overlap.clone(), false))?;
    put(table, &check, query(check.clone(), true))
}

pub(super) fn register(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> Reg {
    let cells = (scene, physics);
    type SphereArgs = (f32, f32, f32, f32, Option<u32>, Option<bool>);
    register_pair(lua, table, cells, "Sphere", |verb, args: SphereArgs| {
        let (cx, cy, cz, radius, mask, triggers) = args;
        let center = check::point(verb, "center", [cx, cy, cz])?;
        let volume = Volume::Sphere(center, check::radius(verb, radius)?);
        Ok((volume, mask, triggers))
    })?;
    // `(cx,cy,cz, hx,hy,hz [, rx,ry,rz [, layer_mask [, query_triggers]]])`:
    // Unity's center, halfExtents, orientation (Euler degrees here).
    type BoxArgs = (
        f32,
        f32,
        f32,
        f32,
        f32,
        f32,
        Option<f32>,
        Option<f32>,
        Option<f32>,
        Option<u32>,
        Option<bool>,
    );
    register_pair(lua, table, cells, "Box", |verb, args: BoxArgs| {
        let (cx, cy, cz, hx, hy, hz, rx, ry, rz, mask, triggers) = args;
        let volume = Volume::Box(
            check::point(verb, "center", [cx, cy, cz])?,
            check::half_extents(verb, [hx, hy, hz])?,
            check::orientation(verb, [rx, ry, rz])?,
        );
        Ok((volume, mask, triggers))
    })?;
    // `(x0,y0,z0, x1,y1,z1, radius [, layer_mask [, query_triggers]])`: the
    // two sphere centers and the radius, Unity's point0 / point1 / radius.
    type CapsuleArgs = (f32, f32, f32, f32, f32, f32, f32, Option<u32>, Option<bool>);
    register_pair(lua, table, cells, "Capsule", |verb, args: CapsuleArgs| {
        let (x0, y0, z0, x1, y1, z1, radius, mask, triggers) = args;
        let volume = Volume::Capsule(
            check::point(verb, "point0", [x0, y0, z0])?,
            check::point(verb, "point1", [x1, y1, z1])?,
            check::radius(verb, radius)?,
        );
        Ok((volume, mask, triggers))
    })
}
