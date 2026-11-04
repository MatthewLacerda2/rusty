//! src/api/nav/query.rs — the `Navigation` path queries (#458): `CalculatePath`,
//! `SamplePosition`, `Raycast` and `GetPathLength`, plus the path table they share
//! with `NavMeshAgent.GetPath`. The first three take an optional trailing area mask
//! (#460, Unity's `areaMask`; every area when omitted).
//!
//! A path is `{status = "complete"|"partial"|"invalid", corners = {{x,y,z}, ...},
//! length = n}`: the Lua face of [`NavPath`].

use glam::Vec3;
use mlua::{Lua, Table};

use super::super::{put, Reg};
use super::areas::mask_arg;
use crate::api::ApiCell;
use crate::navigation::{path_length, NavPath, NavPathStatus, NavigationGraph};

/// Two points and an optional area mask (#460): the path queries' arguments.
type PathArgs = (f32, f32, f32, f32, f32, f32, Option<i64>);

/// A path as its Lua table.
pub(super) fn path_table(
    lua: &Lua,
    status: NavPathStatus,
    corners: &[Vec3],
) -> mlua::Result<Table> {
    let list = lua.create_table()?;
    for (i, c) in corners.iter().enumerate() {
        let t = lua.create_table()?;
        t.set("x", c.x)?;
        t.set("y", c.y)?;
        t.set("z", c.z)?;
        list.set(i + 1, t)?;
    }
    let path = lua.create_table()?;
    path.set("status", status.name())?;
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
pub(super) fn register(lua: &mlua::Lua, table: &Table, nav: ApiCell<NavigationGraph>) -> Reg {
    put(
        table,
        "CalculatePath",
        lua.create_function(move |lua, (fx, fy, fz, tx, ty, tz, mask): PathArgs| {
            let (from, to, nav) = (Vec3::new(fx, fy, fz), Vec3::new(tx, ty, tz), nav.get(lua)?);
            let path = nav.borrow().calculate_path_masked(from, to, mask_arg(mask));
            let NavPath {
                status, corners, ..
            } = path;
            path_table(lua, status, &corners)
        }),
    )?;
    put(
        table,
        "SamplePosition",
        lua.create_function(
            move |lua, (x, y, z, reach, mask): (f32, f32, f32, f32, Option<i64>)| {
                let found = nav.get(lua)?.borrow().sample_position_masked(
                    Vec3::new(x, y, z),
                    reach,
                    mask_arg(mask),
                );
                Ok(match found {
                    Some(p) => (true, p.x, p.y, p.z),
                    None => (false, 0.0, 0.0, 0.0),
                })
            },
        ),
    )?;
    put(
        table,
        "Raycast",
        lua.create_function(move |lua, (fx, fy, fz, tx, ty, tz, mask): PathArgs| {
            let (from, to, nav) = (Vec3::new(fx, fy, fz), Vec3::new(tx, ty, tz), nav.get(lua)?);
            let walk = nav.borrow().raycast_masked(from, to, mask_arg(mask));
            Ok(match walk {
                Some(w) => (w.hit, w.position.x, w.position.y, w.position.z),
                None => (true, from.x, from.y, from.z),
            })
        }),
    )?;
    put(
        table,
        "GetPathLength",
        lua.create_function(|_, path: Table| Ok(path_length(&corners_of(&path)?))),
    )
}
