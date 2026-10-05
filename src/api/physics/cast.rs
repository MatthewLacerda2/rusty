//! src/api/physics/cast.rs — `Physics.Raycast` / `SphereCast` / `RaycastAll`.
//!
//! Query-only line casts through the live rapier world. A hit carries the
//! entity, distance, world point and surface normal (#446), plus the bone it
//! struck and the hierarchy root it belongs to (#464). All share one
//! acceptance test (`accepts`) with the volume and point queries, so every
//! spatial query honours the same ignore / layer-mask rules (#91, #311).

use std::cell::RefCell;

use glam::Vec3;
use mlua::Table;

use super::super::{put, Reg};
use crate::physics::{is_hittable, PhysicsWorld, RayHit};
use crate::scene::{layer_in_mask, Scene};

/// The one acceptance test every spatial query filters colliders through: skip
/// the optional `ignore` entity (e.g. the shooter), require the entity to be
/// hittable ([`is_hittable`]), and — when a layer mask (bit per layer) is given
/// — require the entity's layer bit to be set (#91). Rejected colliders are
/// passed through, never treated as blockers.
pub(super) fn accepts(scene: &Scene, id: u32, ignore: Option<u32>, mask: Option<u32>) -> bool {
    if Some(id) == ignore || !is_hittable(scene, id) {
        return false;
    }
    match mask {
        Some(m) => layer_in_mask(scene.world.layer(id), m),
        None => true,
    }
}

/// A single cast's Lua returns:
/// `hit, entity_id, distance, px, py, pz, nx, ny, nz, bone, bone_name, root`.
/// Each addition is appended (point and normal by #446, the bone and root by
/// #464), so shorter callers are untouched. hit=false ⇒ every number is 0 and
/// the rest nil; `bone`/`bone_name` are nil for a hit off any skeleton.
type CastResult = (
    bool,
    u32,
    f32,
    f32,
    f32,
    f32,
    f32,
    f32,
    f32,
    Option<u32>,
    Option<String>,
    Option<u32>,
);

/// Fold an optional hit into the [`CastResult`] both single casts return.
fn cast_result(scene: &Scene, hit: Option<RayHit>) -> CastResult {
    match hit {
        Some(RayHit {
            id,
            distance,
            point: p,
            normal: n,
        }) => {
            let bone = scene.hit_bone(id);
            (
                true,
                id,
                distance,
                p.x,
                p.y,
                p.z,
                n.x,
                n.y,
                n.z,
                bone,
                bone.and_then(|b| scene.world.name(b).map(|n| n.clone())),
                Some(scene.root_of(id)),
            )
        }
        None => (
            false, 0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, None, None, None,
        ),
    }
}

/// `{x, y, z}` as a Lua table.
fn vec_table(lua: &mlua::Lua, v: Vec3) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("x", v.x)?;
    t.set("y", v.y)?;
    t.set("z", v.z)?;
    Ok(t)
}

/// Set who a hit struck on `t`: `id`, its `bone`/`bone_name` (absent off a
/// skeleton) and `root` (#464). Shared by `RaycastAll` and `RaycastThrough`.
pub(super) fn set_owner(t: &Table, scene: &Scene, id: u32) -> mlua::Result<()> {
    t.set("id", id)?;
    if let Some(bone) = scene.hit_bone(id) {
        t.set("bone", bone)?;
        t.set("bone_name", scene.world.name(bone).map(|n| n.clone()))?;
    }
    t.set("root", scene.root_of(id))
}

/// Set where a hit landed on `t`: `distance`, `point = {x,y,z}`, `normal = {x,y,z}`.
pub(super) fn set_surface(lua: &mlua::Lua, t: &Table, hit: &RayHit) -> mlua::Result<()> {
    t.set("distance", hit.distance)?;
    t.set("point", vec_table(lua, hit.point)?)?;
    t.set("normal", vec_table(lua, hit.normal)?)
}

/// One `RaycastAll` entry: `{id, distance, point = {x,y,z}, normal = {x,y,z},
/// bone, bone_name, root}` — `bone`/`bone_name` absent off a skeleton.
fn hit_table(lua: &mlua::Lua, scene: &Scene, hit: &RayHit) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    set_owner(&t, scene, hit.id)?;
    set_surface(lua, &t, hit)?;
    Ok(t)
}

/// Register `Raycast`, `SphereCast` and `RaycastAll` onto the `Physics` table.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    register_raycast(scope, table, scene, physics)?;
    register_spherecast(scope, table, scene, physics)?;
    register_raycast_all(scope, table, scene, physics)
}

/// `Raycast` — query-only cast returning a [`CastResult`].
fn register_raycast<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    type Args = (f32, f32, f32, f32, f32, f32, Option<u32>, Option<u32>);
    put(
        table,
        "Raycast",
        scope.create_function(|_, (ox, oy, oz, dx, dy, dz, ignore, mask): Args| {
            // The optional trailing `ignore` id is skipped (e.g. the shooter);
            // the optional `mask` limits hits to entities on those layers (#91).
            // Returns `None` (⇒ miss) when no physics world exists yet (edit
            // mode / no Play has built one).
            let physics = physics.borrow();
            let scene = scene.borrow();
            Ok(cast_result(
                &scene,
                physics.as_ref().and_then(|world| {
                    world.raycast_hit(
                        Vec3::new(ox, oy, oz),
                        Vec3::new(dx, dy, dz),
                        f32::MAX,
                        |id| accepts(&scene, id, ignore, mask),
                    )
                }),
            ))
        }),
    )
}

/// `SphereCast` — a raycast with a radius (Unity `Physics.SphereCast`): the
/// first accepted collider touched by a sphere swept along the ray. Same
/// [`CastResult`] as `Raycast`; distance is how far the sphere's center
/// traveled before impact, the point is the contact on the struck surface.
fn register_spherecast<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    type Args = (f32, f32, f32, f32, f32, f32, f32, Option<u32>, Option<u32>);
    put(
        table,
        "SphereCast",
        scope.create_function(|_, (ox, oy, oz, dx, dy, dz, radius, ignore, mask): Args| {
            let physics = physics.borrow();
            let scene = scene.borrow();
            Ok(cast_result(
                &scene,
                physics.as_ref().and_then(|world| {
                    world.sphere_cast_hit(
                        Vec3::new(ox, oy, oz),
                        Vec3::new(dx, dy, dz),
                        radius,
                        f32::MAX,
                        |id| accepts(&scene, id, ignore, mask),
                    )
                }),
            ))
        }),
    )
}

/// `RaycastAll(ox,oy,oz, dx,dy,dz, max_distance [, layer_mask])` — every
/// accepted collider along the ray, as an array of hit tables sorted by
/// distance (ties by entity id). Empty with no live physics world.
fn register_raycast_all<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    type Args = (f32, f32, f32, f32, f32, f32, f32, Option<u32>);
    put(
        table,
        "RaycastAll",
        scope.create_function(|lua, (ox, oy, oz, dx, dy, dz, max, mask): Args| {
            let physics = physics.borrow();
            let scene = scene.borrow();
            let hits = physics.as_ref().map_or_else(Vec::new, |world| {
                world.raycast_all(Vec3::new(ox, oy, oz), Vec3::new(dx, dy, dz), max, |id| {
                    accepts(&scene, id, None, mask)
                })
            });
            let out = lua.create_table()?;
            for (i, hit) in hits.iter().enumerate() {
                out.set(i + 1, hit_table(lua, &scene, hit)?)?;
            }
            Ok(out)
        }),
    )
}
