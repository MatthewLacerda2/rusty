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

fn register_raycast(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> Reg {
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
