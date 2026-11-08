//! src/api/physics/cast/all.rs — `Physics.RaycastAll` and its non-allocating
//! form `RaycastAllNonAlloc` (#1720, Unity's `RaycastNonAlloc`), plus the hit
//! table writers `RaycastThrough` shares.
//!
//! The writers fill a table in place, reusing its `point` / `normal` tables
//! and clearing fields a hit lacks, so the same code writes a fresh entry and
//! refills a caller's reused one.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Table, Value};

use super::super::super::{put, Reg};
use super::filter::{reach, Filter};
use crate::api::ApiCell;
use crate::physics::{PhysicsWorld, RayHit};
use crate::scene::Scene;

/// Write `{x, y, z}` into `t[key]`, reusing the table already there.
fn vec_into(lua: &mlua::Lua, t: &Table, key: &str, v: Vec3) -> mlua::Result<()> {
    let out = match t.raw_get::<Value>(key)? {
        Value::Table(existing) => existing,
        _ => lua.create_table()?,
    };
    out.raw_set("x", v.x)?;
    out.raw_set("y", v.y)?;
    out.raw_set("z", v.z)?;
    t.raw_set(key, out)
}

/// Set who a hit struck on `t`: `id`, its `bone`/`bone_name` (nil off a
/// skeleton, clearing a reused entry's), `root` (#464) and the physics `material`
/// asset it is made of (#1878, nil for an inline one). Every hit table is written
/// through here, so a new cast gets each field by calling it.
pub(in crate::api::physics) fn set_owner(t: &Table, scene: &Scene, id: u32) -> mlua::Result<()> {
    let bone = scene.hit_bone(id);
    t.raw_set("id", id)?;
    t.raw_set("material", hit_material(scene, id))?;
    t.raw_set("bone", bone)?;
    t.raw_set(
        "bone_name",
        bone.and_then(|b| scene.world.name(b).map(|n| n.clone())),
    )?;
    t.raw_set("root", scene.root_of(id))
}

/// The physics material asset a hit on entity `id`'s collider names (#1878):
/// its path while the asset loads, else `None`.
pub(super) fn hit_material(scene: &Scene, id: u32) -> Option<String> {
    let collider = scene.world.collider(id)?;
    crate::scene::physics_material::named(&collider)
}

/// Set where a hit landed on `t`: `distance`, `point = {x,y,z}`, `normal = {x,y,z}`.
pub(in crate::api::physics) fn set_surface(
    lua: &mlua::Lua,
    t: &Table,
    hit: &RayHit,
) -> mlua::Result<()> {
    t.raw_set("distance", hit.distance)?;
    vec_into(lua, t, "point", hit.point)?;
    vec_into(lua, t, "normal", hit.normal)
}

/// Every accepted hit along the ray within `max` (see [`reach`]), nearest
/// first; empty with no live physics world.
fn hits_along(
    physics: &RefCell<Option<PhysicsWorld>>,
    scene: &Scene,
    origin: Vec3,
    dir: Vec3,
    max: f32,
    filter: Filter,
) -> Vec<RayHit> {
    let physics = physics.borrow();
    match (physics.as_ref(), reach(Some(max))) {
        (Some(world), Some(max)) => {
            world.raycast_all(origin, dir, max, |id| filter.accepts(scene, id))
        }
        _ => Vec::new(),
    }
}

/// `RaycastAll(ox,oy,oz, dx,dy,dz, max_distance [, layer_mask [,
/// query_triggers]])` — a fresh array of hit tables `{id, distance, point,
/// normal, bone, bone_name, root, material}`, nearest first (ties by entity id).
/// `RaycastAllNonAlloc(results, <the same arguments>)` writes the same hits
/// into `results` instead and returns their count.
pub(super) fn register(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> Reg {
    type Args = (f32, f32, f32, f32, f32, f32, f32, Option<u32>, Option<bool>);
    put(
        table,
        "RaycastAll",
        lua.create_function(move |lua, args: Args| {
            let (ox, oy, oz, dx, dy, dz, max, mask, triggers) = args;
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let (o, d) = (Vec3::new(ox, oy, oz), Vec3::new(dx, dy, dz));
            let filter = Filter::new(None, mask, triggers);
            let hits = hits_along(&*physics.get(lua)?, &scene, o, d, max, filter);
            let out = lua.create_table_with_capacity(hits.len(), 0)?;
            for (i, hit) in hits.iter().enumerate() {
                let t = lua.create_table()?;
                set_owner(&t, &scene, hit.id)?;
                set_surface(lua, &t, hit)?;
                out.raw_set(i + 1, t)?;
            }
            Ok(out)
        }),
    )?;
    type NonAllocArgs = (
        Table,
        f32,
        f32,
        f32,
        f32,
        f32,
        f32,
        f32,
        Option<u32>,
        Option<bool>,
    );
    put(
        table,
        "RaycastAllNonAlloc",
        lua.create_function(move |lua, args: NonAllocArgs| {
            let (results, ox, oy, oz, dx, dy, dz, max, mask, triggers) = args;
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let (o, d) = (Vec3::new(ox, oy, oz), Vec3::new(dx, dy, dz));
            let filter = Filter::new(None, mask, triggers);
            let hits = hits_along(&*physics.get(lua)?, &scene, o, d, max, filter);
            fill(lua, &results, &scene, &hits)?;
            Ok(hits.len())
        }),
    )
}

/// Write `hits` into `results[1..=n]`, reusing each slot's table when it holds
/// one. Slots past `n` are left as they were (Unity's `RaycastNonAlloc`), and
/// hits past the table's length grow it: a Lua table has no fixed size.
fn fill(lua: &mlua::Lua, results: &Table, scene: &Scene, hits: &[RayHit]) -> mlua::Result<()> {
    for (i, hit) in hits.iter().enumerate() {
        let entry = match results.raw_get::<Value>(i + 1)? {
            Value::Table(t) => t,
            _ => {
                let t = lua.create_table()?;
                results.raw_set(i + 1, &t)?;
                t
            }
        };
        set_owner(&entry, scene, hit.id)?;
        set_surface(lua, &entry, hit)?;
    }
    Ok(())
}
