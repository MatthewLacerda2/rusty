use glam::Vec3;
use mlua::{Lua, Variadic};

use super::{put, Reg};
use crate::api::ApiCell;
use rusty_asset::MapRef;
use rusty_core::settings::decals::DecalLimits;
use rusty_scene::decal::DecalSpec;
use rusty_scene::Scene;

// Metres: the stamp size a script omits, and the floor on the box's depth when it omits that.
const DEFAULT_SIZE: f32 = 0.5;
const DEFAULT_DEPTH: f32 = 0.5;

pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "Spawn",
        lua.create_function(move |lua, args: Variadic<mlua::Value>| { let scene = &*scene.get(lua)?;
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

    register_registry(&table, lua, scene)?;

    lua.globals()
        .set("Decals", table)
        .map_err(|e| e.to_string())
}

fn register_registry(table: &mlua::Table, lua: &mlua::Lua, scene: ApiCell<Scene>) -> Reg {
    // Decals.Remove(id, [fade]) — remove one decal now, or fade it out over `fade`
    // seconds. Returns whether a live decal had that id.
    put(
        table,
        "Remove",
        lua.create_function(move |lua, (decal, fade): (u32, Option<f32>)| {
            Ok(scene
                .get(lua)?
                .borrow_mut()
                .decals
                .remove(decal, fade.unwrap_or(0.0)))
        }),
    )?;

    put(
        table,
        "Count",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().decals.len() as u32)),
    )?;

    put(
        table,
        "Clear",
        lua.create_function(move |lua, opts: Option<mlua::Table>| {
            let keep = clear_keeps_persistent(opts.as_ref())?;
            scene.get(lua)?.borrow_mut().clear_decals(keep);
            Ok(())
        }),
    )?;

    put(
        table,
        "SetLimits",
        lua.create_function(move |lua, opts: mlua::Table| {
            let scene = &*scene.get(lua)?;
            let mut scene = scene.borrow_mut();
            let limits = limits_from(&opts, scene.decals.limits())?;
            scene.decals.set_limits(limits);
            Ok(())
        }),
    )?;

    put(
        table,
        "GetLimits",
        lua.create_function(move |lua, ()| {
            let limits = scene.get(lua)?.borrow().decals.limits();
            let out = lua.create_table()?;
            out.set("per_owner", limits.max_per_owner)?;
            out.set("world", limits.max_world)?;
            out.set("total", limits.max_total)?;
            Ok(out)
        }),
    )
}

fn clear_keeps_persistent(opts: Option<&mlua::Table>) -> mlua::Result<bool> {
    let Some(opts) = opts else {
        return Ok(false);
    };
    for pair in opts.pairs::<String, mlua::Value>() {
        let (key, _) = pair?;
        if key != "persistent" {
            return Err(mlua::Error::RuntimeError(format!(
                "Decals.Clear: unknown option {key:?}; expected persistent"
            )));
        }
    }
    Ok(opt_bool(opts, "Decals.Clear", "persistent")? == Some(false))
}

/// Strict, since mlua's `bool` conversion is Lua truthiness: `"false"` or `0` would read true.
fn opt_bool(opts: &mlua::Table, func: &str, key: &str) -> mlua::Result<Option<bool>> {
    match opts.get::<mlua::Value>(key)? {
        mlua::Value::Nil => Ok(None),
        mlua::Value::Boolean(b) => Ok(Some(b)),
        other => Err(mlua::Error::RuntimeError(format!(
            "{func}: {key} must be true or false, not a {}",
            other.type_name()
        ))),
    }
}

const LIMITS: [&str; 3] = ["per_owner", "world", "total"];

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

const OPTS: [&str; 10] = [
    "size", "depth", "rotation", "color", "texture", "material", "owner", "lifetime", "fade",
    "persist",
];

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
        persist: opt_bool(opts, "Decals.Spawn", "persist")?.unwrap_or(false),
    })
}

fn num_at(args: &[mlua::Value], i: usize) -> mlua::Result<f32> {
    opt_num(args, i)
        .ok_or_else(|| mlua::Error::RuntimeError(format!("Decals.Spawn arg {i} must be a number")))
}

fn opt_num(args: &[mlua::Value], i: usize) -> Option<f32> {
    match args.get(i) {
        Some(mlua::Value::Integer(n)) => Some(*n as f32),
        Some(mlua::Value::Number(n)) => Some(*n as f32),
        _ => None,
    }
}

fn opt_str(args: &[mlua::Value], i: usize) -> Option<String> {
    match args.get(i) {
        Some(mlua::Value::String(s)) => Some(s.to_str().ok()?.to_string()),
        _ => None,
    }
}
