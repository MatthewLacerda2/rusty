//! src/api/decals.rs — `Decals` namespace.
//!
//! Script/REPL/bot control over the scene's box-projector decals (bullet holes,
//! scorch, blood splats). `Spawn` stamps a projector at a world point + surface
//! normal — exactly the pair `Physics.Raycast` already hands a
//! script — so decals stay decoupled from gameplay: the engine never decides
//! *when* to mark a surface, the script does. `Spawn` returns the decal's id, which
//! `Remove` takes (#639); `Clear` wipes them all (level reset). `SetLimits` /
//! `GetLimits` change and read the budgets (#1517) until Stop, which puts the
//! project's back.
//!
//! A decal is a projected *volume*, not a particle: gibs/blood spray are the
//! separate GPU particle system (`Particles`), triggered on the same hit.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Variadic};

use super::{put, Reg};
use crate::asset::MapRef;
use crate::core::settings::decals::DecalLimits;
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
    // The `opts` table form takes the same values by name plus `depth`, the decal
    // `material` (#638), and its lifecycle (#639): the `owner` entity it sticks to,
    // its `lifetime` and its `fade`. Either form returns the decal's id.
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
            let owner = spec.owner;
            scene.spawn_decal(point, normal, spec).ok_or_else(|| {
                mlua::Error::RuntimeError(format!(
                    "Decals.Spawn: no entity {} to stick to",
                    owner.unwrap_or_default()
                ))
            })
        }),
    )?;

    register_registry(&table, scope, scene)?;

    lua.globals()
        .set("Decals", table)
        .map_err(|e| e.to_string())
}

/// The functions over the live decals onto `table`: `Remove`, `Count`, `Clear`, and
/// the budgets' `SetLimits` and `GetLimits` (#1517).
fn register_registry<'scope>(
    table: &mlua::Table,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    // Decals.Remove(id, [fade]) — remove one decal now, or fade it out over `fade`
    // seconds. Returns whether a live decal had that id.
    put(
        table,
        "Remove",
        scope.create_function(|_, (decal, fade): (u32, Option<f32>)| {
            Ok(scene.borrow_mut().decals.remove(decal, fade.unwrap_or(0.0)))
        }),
    )?;

    // Decals.Count() — number of live decals, counting any still fading out.
    put(
        table,
        "Count",
        scope.create_function(|_, ()| Ok(scene.borrow().decals.len() as u32)),
    )?;

    // Decals.Clear() — drop every live decal (e.g. on level reset).
    put(
        table,
        "Clear",
        scope.create_function(|_, ()| {
            scene.borrow_mut().clear_decals();
            Ok(())
        }),
    )?;

    // Decals.SetLimits{ per_owner =, world =, total = } — change the budgets for the
    // rest of this Play; a key left out keeps its value. Lowering one fades the
    // oldest excess out at once.
    put(
        table,
        "SetLimits",
        scope.create_function(|_, opts: mlua::Table| {
            let mut scene = scene.borrow_mut();
            let limits = limits_from(&opts, scene.decals.limits())?;
            scene.decals.set_limits(limits);
            Ok(())
        }),
    )?;

    // Decals.GetLimits() — the budgets in force, as { per_owner, world, total }.
    put(
        table,
        "GetLimits",
        scope.create_function(|lua, ()| {
            let limits = scene.borrow().decals.limits();
            let out = lua.create_table()?;
            out.set("per_owner", limits.max_per_owner)?;
            out.set("world", limits.max_world)?;
            out.set("total", limits.max_total)?;
            Ok(out)
        }),
    )
}

/// The keys `Decals.SetLimits` takes.
const LIMITS: [&str; 3] = ["per_owner", "world", "total"];

/// `opts` over `limits`: each key it names replaces that budget. An unknown key is
/// an error naming the valid ones.
fn limits_from(opts: &mlua::Table, mut limits: DecalLimits) -> mlua::Result<DecalLimits> {
    for pair in opts.pairs::<String, mlua::Value>() {
        let (key, _) = pair?;
        if !LIMITS.contains(&key.as_str()) {
            return Err(mlua::Error::RuntimeError(format!(
                "Decals.SetLimits: unknown limit {key:?}; expected one of {}",
                LIMITS.join(", ")
            )));
        }
    }
    let fields = [
        (LIMITS[0], &mut limits.max_per_owner),
        (LIMITS[1], &mut limits.max_world),
        (LIMITS[2], &mut limits.max_total),
    ];
    for (key, field) in fields {
        if let Some(value) = opts.get::<Option<u32>>(key)? {
            *field = value;
        }
    }
    Ok(limits)
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
        texture: opt_str(args, 7).map(MapRef::from),
        ..DecalSpec::default()
    }
}

/// The keys an `opts` table may hold.
const OPTS: [&str; 9] = [
    "size", "depth", "rotation", "color", "texture", "material", "owner", "lifetime", "fade",
];

/// The `opts` table form: the positional values by name, plus `depth`, the decal
/// `material` and the lifecycle (`owner`, `lifetime`, `fade`). An unknown key is an
/// error naming the valid ones.
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
        texture: opts.get::<Option<String>>("texture")?.map(MapRef::from),
        material: opts.get("material")?,
        owner: opts.get("owner")?,
        lifetime: opts.get("lifetime")?,
        fade: opts.get::<Option<f32>>("fade")?.unwrap_or(0.0),
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
