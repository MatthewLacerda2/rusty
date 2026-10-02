//! src/api/nav/query.rs — the `Navigation` path queries (#458): `CalculatePath`,
//! `SamplePosition`, `Raycast` and `GetPathLength`, plus the path table they share
//! with `NavMeshAgent.GetPath`.
//!
//! A path is `{status = "complete"|"partial"|"invalid", corners = {{x,y,z}, ...},
//! length = n}`: the Lua face of [`NavPath`].

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::super::{put, Reg};
use crate::navigation::{path_length, NavPath, NavPathStatus, NavigationGraph};

/// A path as its Lua table.
pub(super) fn path_table<'lua>(
    lua: &'lua Lua,
    status: NavPathStatus,
    corners: &[Vec3],
) -> mlua::Result<Table<'lua>> {
    let list = lua.create_table()?;
    for (i, c) in corners.iter().enumerate() {
        let t = lua.create_table()?;
        t.set("x", c.x)?;
        t.set("y", c.y)?;
        t.set("z", c.z)?;
        list.set(i + 1, t)?;
    }
    let path = lua.create_table()?;
    path.set("status", status.as_str())?;
    path.set("corners", list)?;
    path.set("length", path_length(corners))?;
    Ok(path)
}

/// The corners of a path table, read back for `GetPathLength`.
fn corners_of(path: &Table) -> mlua::Result<Vec<Vec3>> {
    let list: Table = path.get("corners")?;
    list.sequence_values::<Table>()
        .map(|c| {
            let c = c?;
            Ok(Vec3::new(c.get("x")?, c.get("y")?, c.get("z")?))
        })
        .collect()
}

/// Register the queries onto the `Navigation` table.
pub(super) fn register<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    nav: &'scope RefCell<NavigationGraph>,
) -> Reg {
    put(
        table,
        "CalculatePath",
        scope.create_function(
            |lua, (fx, fy, fz, tx, ty, tz): (f32, f32, f32, f32, f32, f32)| {
                let NavPath { status, corners } = nav
                    .borrow()
                    .calculate_path(Vec3::new(fx, fy, fz), Vec3::new(tx, ty, tz));
                path_table(lua, status, &corners)
            },
        ),
    )?;
    put(
        table,
        "SamplePosition",
        scope.create_function(|_, (x, y, z, max_distance): (f32, f32, f32, f32)| {
            let found = nav
                .borrow()
                .sample_position(Vec3::new(x, y, z), max_distance);
            Ok(match found {
                Some(p) => (true, p.x, p.y, p.z),
                None => (false, 0.0, 0.0, 0.0),
            })
        }),
    )?;
    put(
        table,
        "Raycast",
        scope.create_function(
            |_, (fx, fy, fz, tx, ty, tz): (f32, f32, f32, f32, f32, f32)| {
                let from = Vec3::new(fx, fy, fz);
                let walk = nav.borrow().raycast(from, Vec3::new(tx, ty, tz));
                Ok(match walk {
                    Some(w) => (w.hit, w.position.x, w.position.y, w.position.z),
                    None => (true, from.x, from.y, from.z),
                })
            },
        ),
    )?;
    put(
        table,
        "GetPathLength",
        scope.create_function(|_, path: Table| Ok(path_length(&corners_of(&path)?))),
    )
}
