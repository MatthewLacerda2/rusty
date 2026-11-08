//! `Debug.DrawLine` / `Debug.DrawRay` (#1882): Unity's debug lines, held on the
//! scene for a duration in sim time counted in fixed ticks
//! (`scene::runtime::debug_lines`) and drawn by the gizmo pass: always in the
//! editor's Scene view, and over the game with its gizmos on (the Game view's
//! toggle, `Harness.Screenshot(path, { gizmos = true })`). Draw-only: nothing in
//! the sim reads them back.
//!
//! Vectors and the colour cross flat, as everywhere in the API:
//! `(x, y, z, x, y, z [, r, g, b, a [, duration]])`, Unity's order and defaults
//! (white; a duration of 0, one frame).

use glam::Vec3;
use mlua::{Lua, MultiValue, Table};

use crate::api::{put, ApiCell, Reg};
use crate::scene::runtime::debug_lines::duration_ticks;
use crate::scene::Scene;

/// Register `Debug.DrawLine` and `Debug.DrawRay` onto `table`.
pub(super) fn register(lua: &Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "DrawLine",
        lua.create_function(move |lua, args: MultiValue| {
            let line = parse("Debug.DrawLine", args)?;
            scene
                .get(lua)?
                .borrow_mut()
                .debug_lines
                .push(line.a, line.b, line.color, line.ticks);
            Ok(())
        }),
    )?;
    put(
        table,
        "DrawRay",
        lua.create_function(move |lua, args: MultiValue| {
            let ray = parse("Debug.DrawRay", args)?;
            scene.get(lua)?.borrow_mut().debug_lines.push(
                ray.a,
                ray.a + ray.b,
                ray.color,
                ray.ticks,
            );
            Ok(())
        }),
    )
}

/// One call's arguments: two vectors, the colour and the duration in ticks.
#[derive(Debug, PartialEq)]
struct Drawn {
    a: Vec3,
    b: Vec3,
    color: [f32; 4],
    ticks: u32,
}

/// Read `(x, y, z, x, y, z [, r, g, b, a [, duration]])`: six, ten or eleven numbers.
fn parse(verb: &str, args: MultiValue) -> mlua::Result<Drawn> {
    let bad = |why: String| mlua::Error::RuntimeError(format!("{verb}: {why}"));
    let mut n = Vec::with_capacity(args.len());
    for (i, value) in args.into_iter().enumerate() {
        let number = match value {
            mlua::Value::Number(x) => x as f32,
            mlua::Value::Integer(x) => x as f32,
            other => {
                let kind = other.type_name();
                return Err(bad(format!(
                    "argument {} must be a number, not {kind}",
                    i + 1
                )));
            }
        };
        n.push(number);
    }
    if !matches!(n.len(), 6 | 10 | 11) {
        let got = n.len();
        let want = "(x, y, z, x, y, z [, r, g, b, a [, duration]])";
        return Err(bad(format!("takes {want}: 6, 10 or 11 numbers, got {got}")));
    }
    let color = match n.len() {
        6 => [1.0; 4],
        _ => [n[6], n[7], n[8], n[9]],
    };
    let duration = n.get(10).copied().unwrap_or(0.0);
    Ok(Drawn {
        a: Vec3::new(n[0], n[1], n[2]),
        b: Vec3::new(n[3], n[4], n[5]),
        color,
        ticks: duration_ticks(duration, crate::time::FIXED_DELTA_TIME),
    })
}

#[cfg(test)]
#[path = "draw_tests.rs"]
mod tests;
