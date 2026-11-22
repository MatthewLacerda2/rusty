use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::transform::{euler_degrees, from_euler_degrees};
use rusty_ecs::World;
use rusty_scene::Scene;

type Read = fn(&World, u32) -> Option<Vec3>;
type Write = fn(&mut World, u32, Vec3);
type Map = fn(&World, u32, Vec3) -> Option<Vec3>;

pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    for (name, read, default) in READS {
        getter(lua, &table, scene, name, *read, *default)?;
    }
    for (name, write) in WRITES {
        setter(lua, &table, scene, name, *write)?;
    }
    for (name, map) in MAPS {
        mapper(lua, &table, scene, name, *map)?;
    }
    register_move_towards(lua, &table, scene)?;
    lua.globals()
        .set("Transform", table)
        .map_err(|e| e.to_string())
}

const ZERO: Vec3 = Vec3::ZERO;

const READS: &[(&str, Read, Vec3)] = &[
    ("GetPosition", |w, id| Some(w.world_position(id)), ZERO),
    (
        "GetLocalPosition",
        |w, id| Some(w.transform(id)?.position),
        ZERO,
    ),
    (
        "GetRotation",
        |w, id| Some(euler_degrees(w.world_rotation(id))),
        ZERO,
    ),
    (
        "GetLocalRotation",
        |w, id| Some(w.transform(id)?.euler_angles()),
        ZERO,
    ),
    (
        "GetLocalScale",
        |w, id| Some(w.transform(id)?.scale),
        Vec3::ONE,
    ),
    ("GetLossyScale", |w, id| Some(w.lossy_scale(id)), Vec3::ONE),
];

const WRITES: &[(&str, Write)] = &[
    ("SetPosition", |w, id, v| {
        w.set_world_position(id, v);
    }),
    ("SetLocalPosition", |w, id, v| {
        if let Some(mut t) = w.transform_mut(id) {
            t.position = v;
        }
    }),
    ("SetRotation", |w, id, v| {
        w.set_world_rotation(id, from_euler_degrees(v));
    }),
    ("SetLocalRotation", |w, id, v| {
        if let Some(mut t) = w.transform_mut(id) {
            t.set_euler_angles(v);
        }
    }),
    ("SetLocalScale", |w, id, v| {
        if let Some(mut t) = w.transform_mut(id) {
            t.scale = v;
        }
    }),
];

/// The space maps, `(id, x, y, z) -> x, y, z`. A missing entity maps the vector as
/// the identity; `InverseTransformPoint` gives `nil` under a singular world matrix.
const MAPS: &[(&str, Map)] = &[
    ("TransformPoint", |w, id, p| Some(w.transform_point(id, p))),
    ("TransformDirection", |w, id, d| {
        Some(w.transform_direction(id, d))
    }),
    ("InverseTransformPoint", |w, id, p| {
        w.inverse_transform_point(id, p)
    }),
];

fn getter(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    name: &str,
    read: Read,
    default: Vec3,
) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        let v = read(&scene.get(lua)?.borrow().world, id.read()).unwrap_or(default);
        Ok((v.x, v.y, v.z))
    });
    put(table, name, f)
}

fn setter(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    name: &str,
    write: Write,
) -> Reg {
    let verb = name.to_string();
    let f = lua.create_function(move |lua, (id, x, y, z): (EntityArg, f32, f32, f32)| {
        let id = id.live("Transform", &verb)?;
        write(
            &mut scene.get(lua)?.borrow_mut().world,
            id,
            Vec3::new(x, y, z),
        );
        Ok(())
    });
    put(table, name, f)
}

fn mapper(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    name: &str,
    map: Map,
) -> Reg {
    let f = lua.create_function(move |lua, (id, x, y, z): (EntityArg, f32, f32, f32)| {
        let v = map(
            &scene.get(lua)?.borrow().world,
            id.read(),
            Vec3::new(x, y, z),
        );
        Ok(v.map_or((None, None, None), |v| (Some(v.x), Some(v.y), Some(v.z))))
    });
    put(table, name, f)
}

/// `MoveTowards` — step the world position toward a world target, snapping when
/// within `step`: glam's `move_towards` (Unity's `Vector3.MoveTowards`), which a
/// negative `step` walks away from the target.
fn register_move_towards(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    let f = lua.create_function(
        move |lua, (id, tx, ty, tz, step): (EntityArg, f32, f32, f32, f32)| {
            let id = id.live("Transform", "MoveTowards")?;
            let scene = &*scene.get(lua)?;
            let world = &mut scene.borrow_mut().world;
            let at = world
                .world_position(id)
                .move_towards(Vec3::new(tx, ty, tz), step);
            world.set_world_position(id, at);
            Ok(())
        },
    );
    put(table, "MoveTowards", f)
}
