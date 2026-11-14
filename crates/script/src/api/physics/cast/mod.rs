//! crates/script/src/api/physics/cast/mod.rs — `Physics.Raycast`, the shape casts (in
//! `sweep`: `SphereCast` / `CapsuleCast` / `BoxCast`, #1881), and (in `all`)
//! `RaycastAll` / `RaycastAllNonAlloc` / `SphereCastAll`.
//!
//! Query-only line casts through the live rapier world. A hit carries the
//! entity, distance, world point and surface normal (#446), plus the bone it
//! struck and the hierarchy root it belongs to (#464). All share one
//! acceptance test (`filter::Filter`) with the volume and point queries, so
//! every spatial query honours the same ignore / layer-mask / trigger rules
//! (#91, #311, #1720), and one `max_distance` rule (`filter::reach`).

mod all;
pub(super) mod check;
mod filter;
mod sweep;

use glam::Vec3;
use mlua::Table;

use super::super::{put, Reg};
use crate::api::ApiCell;
use rusty_physics::{PhysicsWorld, RayHit};
use rusty_scene::Scene;

pub(super) use all::{set_owner, set_surface};
pub(super) use filter::{reach, Filter};

/// A single cast's Lua returns:
/// `hit, entity_id, distance, px, py, pz, nx, ny, nz, bone, bone_name, root,
/// material`. Each addition is appended (point and normal by #446, the bone and
/// root by #464, the physics material asset by #1878), so shorter callers are
/// untouched. hit=false ⇒ every number is 0 and the rest nil; `bone`/`bone_name`
/// are nil for a hit off any skeleton, `material` for an inline material.
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
    Option<String>,
);

/// Fold an optional hit into the [`CastResult`] every single cast returns.
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
                all::hit_material(scene, id),
            )
        }
        None => (
            false, 0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, None, None, None, None,
        ),
    }
}

/// Register `Raycast`, the shape casts and the multi-hit casts onto the
/// `Physics` table.
pub(super) fn register(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> Reg {
    register_raycast(lua, table, scene, physics)?;
    sweep::register(lua, table, scene, physics)?;
    all::register(lua, table, scene, physics)
}

/// `Raycast(ox,oy,oz, dx,dy,dz [, ignore_id [, layer_mask [, max_distance [,
/// query_triggers]]]])` — query-only cast returning a [`CastResult`]. A miss
/// with no physics world yet (edit mode / no Play has built one).
fn register_raycast(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> Reg {
    // The ray, then `ignore_id, layer_mask, max_distance, query_triggers`.
    type Args = (
        f32,
        f32,
        f32,
        f32,
        f32,
        f32,
        Option<u32>,
        Option<u32>,
        Option<f32>,
        Option<bool>,
    );
    put(
        table,
        "Raycast",
        lua.create_function(move |lua, args: Args| {
            let (ox, oy, oz, dx, dy, dz, ignore, mask, max, triggers) = args;
            let filter = Filter::new(ignore, mask, triggers);
            let scene = &*scene.get(lua)?;
            let physics = &*physics.get(lua)?;
            let physics = physics.borrow();
            let scene = scene.borrow();
            let hit = physics.as_ref().zip(reach(max)).and_then(|(world, max)| {
                world.raycast_hit(Vec3::new(ox, oy, oz), Vec3::new(dx, dy, dz), max, |id| {
                    filter.accepts(&scene, id)
                })
            });
            Ok(cast_result(&scene, hit))
        }),
    )
}
