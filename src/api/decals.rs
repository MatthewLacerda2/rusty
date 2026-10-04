//! src/api/decals.rs — `Decals` namespace.
//!
//! Script/REPL/bot control over the scene's box-projector decals (bullet holes,
//! scorch, blood splats). `Spawn` stamps a projector at a world point + surface
//! normal — exactly the pair `Physics.Raycast` already hands a
//! script — so decals stay decoupled from gameplay: the engine never decides
//! *when* to mark a surface, the script does. `Clear` wipes them (level reset).
//!
//! A decal is a projected *volume*, not a particle: gibs/blood spray are the
//! separate GPU particle system (`Particles`), triggered on the same hit.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Variadic};

use super::{put, Reg};
use crate::scene::decal::DecalSpec;
use crate::scene::Scene;

/// Default stamp size (world units) and projection depth when a script omits them.
const DEFAULT_SIZE: f32 = 0.5;
const DEFAULT_DEPTH: f32 = 0.5;

/// Register the `Decals` namespace onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    // Decals.Spawn(x,y,z, nx,ny,nz, [size], [texture], [rotation_deg], [r,g,b,a])
    // Decals.Spawn(x,y,z, nx,ny,nz, opts)
    //
    // Stamp a box-projector decal at world point (x,y,z) facing along the surface
    // normal (nx,ny,nz). Trailing args are optional: `size` (stamp width/height,
    // default 0.5), `texture` (albedo path, default a solid square), `rotation_deg`
    // (spin around the projection axis), and an r,g,b,a tint (default opaque white).
    // The box depth tracks `size` so the projector reaches through typical geometry.
    // The `opts` table form takes the same values by name plus `depth` and the
    // decal `material` (#638), the library material the decal stamps.
    put(
        &table,
        "Spawn",
        scope.create_function(|_, args: Variadic<mlua::Value>| {
            let (point, normal) = hit_from(&args)?;
            let spec = match args.get(6) {
                Some(mlua::Value::Table(opts)) => spec_from_opts(opts)?,
                _ => spec_from_positional(&args),
            };
            let mut scene = scene.borrow_mut();
            if let Some(name) = spec.material.as_deref() {
                if !scene.materials.contains_key(name) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Decals.Spawn: no material named {name:?} (define it with Material.DefineAsset)"
                    )));
                }
            }
            scene.spawn_decal(point, normal, spec);
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

/// The 6 required floats: the hit point and the surface normal.
fn hit_from(args: &[mlua::Value]) -> mlua::Result<(Vec3, Vec3)> {
    if args.len() < 6 {
        return Err(mlua::Error::RuntimeError(
            "Decals.Spawn expects at least x,y,z,nx,ny,nz".to_string(),
        ));
    }
    let f = |i: usize| -> mlua::Result<f32> { num_at(args, i) };
    Ok((
        Vec3::new(f(0)?, f(1)?, f(2)?),
        Vec3::new(f(3)?, f(4)?, f(5)?),
    ))
}

/// The positional form's optional `size`, `texture`, `rotation_deg` and r,g,b,a.
fn spec_from_positional(args: &[mlua::Value]) -> DecalSpec {
    let size = opt_num(args, 6).unwrap_or(DEFAULT_SIZE);
    DecalSpec {
        size,
        depth: size.max(DEFAULT_DEPTH),
        rotation_deg: opt_num(args, 8).unwrap_or(0.0),
        color: [9, 10, 11, 12].map(|i| opt_num(args, i).unwrap_or(1.0)),
        texture: opt_str(args, 7),
        material: None,
    }
}

/// The keys an `opts` table may hold.
const OPTS: [&str; 6] = ["size", "depth", "rotation", "color", "texture", "material"];

/// The `opts` table form: the positional values by name, plus `depth` and the
/// decal `material`. An unknown key is an error naming the valid ones.
fn spec_from_opts(opts: &mlua::Table) -> mlua::Result<DecalSpec> {
    for pair in opts.pairs::<String, mlua::Value>() {
        let (key, _) = pair?;
        if !OPTS.contains(&key.as_str()) {
            return Err(mlua::Error::RuntimeError(format!(
                "Decals.Spawn: unknown option {key:?}; expected one of {}",
                OPTS.join(", ")
            )));
        }
    }
    let size = opts.get::<Option<f32>>("size")?.unwrap_or(DEFAULT_SIZE);
    let color = match opts.get::<Option<Vec<f32>>>("color")? {
        Some(c) => [0, 1, 2, 3].map(|i| c.get(i).copied().unwrap_or(1.0)),
        None => [1.0; 4],
    };
    Ok(DecalSpec {
        size,
        depth: opts
            .get::<Option<f32>>("depth")?
            .unwrap_or(size.max(DEFAULT_DEPTH)),
        rotation_deg: opts.get::<Option<f32>>("rotation")?.unwrap_or(0.0),
        color,
        texture: opts.get("texture")?,
        material: opts.get("material")?,
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
