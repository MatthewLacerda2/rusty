//! src/api/physics/cast/sweep.rs — the single shape casts: `Physics.SphereCast`,
//! `CapsuleCast` and `BoxCast` (#311, #1881).
//!
//! Each sweeps a [`Sweep`] along a direction and returns the first accepted
//! hit as a [`CastResult`], with `Raycast`'s four trailing options
//! (`ignore_id, layer_mask, max_distance, query_triggers`). Unity's argument
//! order is kept, flattened into numbers, except that `SphereCast` takes its
//! radius after the direction, as it always has here. Bad sizes and
//! directions are Lua errors ([`check`](super::check)).

use glam::Vec3;
use mlua::Table;

use super::super::super::{put, Reg};
use super::check;
use super::{cast_result, reach, CastResult, Filter};
use crate::api::ApiCell;
use crate::physics::{PhysicsWorld, Sweep};
use crate::scene::Scene;

/// `Raycast`'s trailing options: `ignore_id, layer_mask, max_distance,
/// query_triggers`.
type Options = (Option<u32>, Option<u32>, Option<f32>, Option<bool>);

/// The first hit of `sweep` along `dir` under `options`; a miss with no live
/// physics world (edit mode) or a negative / NaN `max_distance`.
fn first_hit(
    lua: &mlua::Lua,
    (scene, physics): (ApiCell<Scene>, ApiCell<Option<PhysicsWorld>>),
    sweep: Sweep,
    dir: Vec3,
    (ignore, mask, max, triggers): Options,
) -> mlua::Result<CastResult> {
    let filter = Filter::new(ignore, mask, triggers);
    let scene = &*scene.get(lua)?;
    let physics = &*physics.get(lua)?;
    let physics = physics.borrow();
    let scene = scene.borrow();
    let hit = physics.as_ref().zip(reach(max)).and_then(|(world, max)| {
        world.shape_cast_hit(sweep, dir, max, |id| filter.accepts(&scene, id))
    });
    Ok(cast_result(&scene, hit))
}

/// Register `SphereCast`, `CapsuleCast` and `BoxCast` onto `Physics`.
pub(super) fn register(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> Reg {
    let cells = (scene, physics);
    // `SphereCast(ox,oy,oz, dx,dy,dz, radius, <options>)`: a raycast with a
    // radius. Distance is how far the sphere's center traveled.
    type SphereArgs = (f32, f32, f32, f32, f32, f32, f32, Options);
    put(
        table,
        "SphereCast",
        lua.create_function(move |lua, args: SphereArgs| {
            let (ox, oy, oz, dx, dy, dz, radius, options) = args;
            let verb = "SphereCast";
            let sweep = Sweep::Sphere {
                center: check::point(verb, "origin", [ox, oy, oz])?,
                radius: check::radius(verb, radius)?,
            };
            let dir = check::direction(verb, [dx, dy, dz])?;
            first_hit(lua, cells, sweep, dir, options)
        }),
    )?;
    // `CapsuleCast(x0,y0,z0, x1,y1,z1, radius, dx,dy,dz, <options>)`: the two
    // sphere centers, the radius, then the direction (Unity's order).
    type CapsuleArgs = (f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, Options);
    put(
        table,
        "CapsuleCast",
        lua.create_function(move |lua, args: CapsuleArgs| {
            let (x0, y0, z0, x1, y1, z1, radius, dx, dy, dz, options) = args;
            let verb = "CapsuleCast";
            let sweep = Sweep::Capsule {
                a: check::point(verb, "point1", [x0, y0, z0])?,
                b: check::point(verb, "point2", [x1, y1, z1])?,
                radius: check::radius(verb, radius)?,
            };
            let dir = check::direction(verb, [dx, dy, dz])?;
            first_hit(lua, cells, sweep, dir, options)
        }),
    )?;
    register_box(lua, table, cells)
}

/// `BoxCast(cx,cy,cz, hx,hy,hz, dx,dy,dz [, rx,ry,rz [, <options>]])`: center,
/// half-extents, direction, then the optional orientation in Euler degrees
/// (Unity's order; absent is no rotation).
fn register_box(
    lua: &mlua::Lua,
    table: &Table,
    cells: (ApiCell<Scene>, ApiCell<Option<PhysicsWorld>>),
) -> Reg {
    // The center, half-extents and direction, the Euler angles, the options.
    type BoxArgs = (
        f32,
        f32,
        f32,
        f32,
        f32,
        f32,
        f32,
        f32,
        f32,
        Option<f32>,
        Option<f32>,
        Option<f32>,
        Options,
    );
    put(
        table,
        "BoxCast",
        lua.create_function(move |lua, args: BoxArgs| {
            let (cx, cy, cz, hx, hy, hz, dx, dy, dz, rx, ry, rz, options) = args;
            let verb = "BoxCast";
            let sweep = Sweep::Box {
                center: check::point(verb, "center", [cx, cy, cz])?,
                half_extents: check::half_extents(verb, [hx, hy, hz])?,
                rotation: check::orientation(verb, [rx, ry, rz])?,
            };
            let dir = check::direction(verb, [dx, dy, dz])?;
            first_hit(lua, cells, sweep, dir, options)
        }),
    )
}
