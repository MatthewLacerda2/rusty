//! crates/script/src/api/physics/volume.rs — `Physics.Overlap*` / `Physics.Check*`.
//!
//! Area queries over the live rapier world (#311): which entities' colliders
//! are inside a sphere/box/capsule *right now* (Unity `Physics.OverlapSphere` /
//! `OverlapBox` / `OverlapCapsule`), plus the boolean `CheckSphere` /
//! `CheckBox` / `CheckCapsule` fast-path for "is anything there?". The box
//! takes an optional orientation in Euler degrees (#1881). Overlap results are
//! arrays of entity ids sorted ascending. Every verb takes an optional trailing
//! `query_triggers` after `layer_mask` (#1720; absent hits triggers); with no
//! live physics world (edit mode) every overlap is empty and every check is
//! false. A negative or non-finite size is a Lua error (`cast::check`).

use glam::{Quat, Vec3};
use mlua::{Function, Table};

use super::super::{put, Reg};
use super::cast::{check, Filter};
use crate::api::ApiCell;
use rusty_physics::PhysicsWorld;
use rusty_scene::Scene;

/// A volume to test, with its sizes checked.
#[derive(Clone, Copy)]
enum Volume {
    Sphere(Vec3, f32),
    Box(Vec3, Vec3, Quat),
    Capsule(Vec3, Vec3, f32),
}

/// A verb's checked volume and its `layer_mask, query_triggers`.
type Query = (Volume, Option<u32>, Option<bool>);

/// The two cells every volume query reads.
type Cells = (ApiCell<Scene>, ApiCell<Option<PhysicsWorld>>);

/// Ids of the accepted colliders overlapping `volume`; empty with no world.
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

/// Register `Overlap<name>` (the id array) and `Check<name>` (whether it is
/// non-empty), both reading their volume with `volume`.
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

/// Register the overlap/check family onto the `Physics` table.
pub(super) fn register(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> Reg {
    let cells = (scene, physics);
    // `(cx,cy,cz, radius [, layer_mask [, query_triggers]])`.
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
