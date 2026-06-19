//! src/api/decals.rs — `Decals` namespace.
//!
//! Script/REPL/bot control over the scene's box-projector decals (bullet holes,
//! scorch, blood splats). `Spawn` stamps a projector at a world point + surface
//! normal — exactly the pair `Physics.Raycast`/`Physics.Shoot` already hands a
//! script — so decals stay decoupled from gameplay: the engine never decides
//! *when* to mark a surface, the script does. `Clear` wipes them (level reset).
//!
//! A decal is a projected *volume*, not a particle: gibs/blood spray are the
//! separate GPU particle system (`Particles`), triggered on the same hit.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Variadic};

use super::{put, Reg};
use crate::scene::Scene;

/// Default stamp size (world units) and projection depth when a script omits them.
const DEFAULT_SIZE: f32 = 0.5;
const DEFAULT_DEPTH: f32 = 0.5;

/// Register the `Decals` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    // Decals.Spawn(x,y,z, nx,ny,nz, [size], [texture], [rotation_deg], [r,g,b,a])
    //
    // Stamp a box-projector decal at world point (x,y,z) facing along the surface
    // normal (nx,ny,nz). Trailing args are optional: `size` (stamp width/height,
    // default 0.5), `texture` (sprite path, default checker), `rotation_deg` (spin
    // around the projection axis), and an r,g,b,a tint (default opaque white). The
    // box depth tracks `size` so the projector reaches through typical geometry.
    put(
        &table,
        "Spawn",
        scope.create_function(|_, args: Variadic<mlua::Value>| {
            let p = spawn_params_from(&args)?;
            scene.borrow_mut().spawn_decal(
                p.point,
                p.normal,
                p.size,
                p.size.max(DEFAULT_DEPTH),
                p.rotation_deg,
                p.color,
                p.texture,
            );
            Ok(())
        }),
    )?;

    // Decals.Count() — number of live decals (after FIFO eviction).
    put(
        &table,
        "Count",
        scope.create_function(|_, ()| Ok(scene.borrow().decals.len() as u32)),
    )?;

    // Decals.Clear() — drop every live decal (e.g. on level reset).
    put(
        &table,
        "Clear",
        scope.create_function(|_, ()| {
            scene.borrow_mut().clear_decals();
            Ok(())
        }),
    )?;

    lua.globals()
        .set("Decals", table)
        .map_err(|e| e.to_string())
}

/// Parsed `Decals.Spawn` arguments.
struct SpawnParams {
    point: Vec3,
    normal: Vec3,
    size: f32,
    rotation_deg: f32,
    color: [f32; 4],
    texture: Option<String>,
}

/// Parse the variadic `Spawn` args: 6 required floats (point + normal) then the
/// optional `size`, `texture`, `rotation_deg`, and r,g,b,a tint.
fn spawn_params_from(args: &[mlua::Value]) -> mlua::Result<SpawnParams> {
    if args.len() < 6 {
        return Err(mlua::Error::RuntimeError(
            "Decals.Spawn expects at least x,y,z,nx,ny,nz".to_string(),
        ));
    }
    let f = |i: usize| -> mlua::Result<f32> { num_at(args, i) };
    let point = Vec3::new(f(0)?, f(1)?, f(2)?);
    let normal = Vec3::new(f(3)?, f(4)?, f(5)?);
    let size = opt_num(args, 6).unwrap_or(DEFAULT_SIZE);
    let texture = opt_str(args, 7);
    let rotation_deg = opt_num(args, 8).unwrap_or(0.0);
    let color = [
        opt_num(args, 9).unwrap_or(1.0),
        opt_num(args, 10).unwrap_or(1.0),
        opt_num(args, 11).unwrap_or(1.0),
        opt_num(args, 12).unwrap_or(1.0),
    ];
    Ok(SpawnParams {
        point,
        normal,
        size,
        rotation_deg,
        color,
        texture,
    })
}

/// Required numeric arg at `i` (errors if absent/non-numeric).
fn num_at(args: &[mlua::Value], i: usize) -> mlua::Result<f32> {
    opt_num(args, i)
        .ok_or_else(|| mlua::Error::RuntimeError(format!("Decals.Spawn arg {i} must be a number")))
}

/// Optional numeric arg at `i` (None if missing or not a number).
fn opt_num(args: &[mlua::Value], i: usize) -> Option<f32> {
    match args.get(i) {
        Some(mlua::Value::Integer(n)) => Some(*n as f32),
        Some(mlua::Value::Number(n)) => Some(*n as f32),
        _ => None,
    }
}

/// Optional string arg at `i` (None if missing or not a string).
fn opt_str(args: &[mlua::Value], i: usize) -> Option<String> {
    match args.get(i) {
        Some(mlua::Value::String(s)) => Some(s.to_str().ok()?.to_string()),
        _ => None,
    }
}
