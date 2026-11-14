//! crates/script/src/api/physics/material.rs — physics material assets (#1878): `SetMaterial` /
//! `GetMaterial` name a collider's shared asset (Unity's `Collider.sharedMaterial`),
//! and `CreateMaterialAsset` / `GetMaterialAsset` / `SetMaterialAsset` make and edit
//! the asset file itself, the verbs the inspector's Collider card has.
//!
//! The asset and its cache are `scene::physics_material`'s; a field table is
//! `{friction, bounciness, friction_combine, bounce_combine}`, each optional.

use mlua::{Lua, Table};

use super::super::{put, Reg};
use super::collider::combine_mode;
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::PhysicsMaterial;
use rusty_scene::authoring::physics::collider as collider_ops;
use rusty_scene::physics_material;
use rusty_scene::Scene;

fn err(msg: String) -> mlua::Error {
    mlua::Error::RuntimeError(msg)
}

/// Register the five verbs onto the `Physics` table.
pub(super) fn register(lua: &Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    register_reference(lua, table, scene)?;
    register_asset(lua, table)
}

/// `SetMaterial(id, path | nil)` and `GetMaterial(id)` → the path or nil.
fn register_reference(lua: &Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetMaterial",
        lua.create_function(move |lua, (id, path): (EntityArg, Option<String>)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Physics", "SetMaterial")?;
            if let Some(mut c) = scene.borrow_mut().world.collider_mut(id) {
                collider_ops::set_material_asset(&mut c, path)
                    .map_err(|e| err(format!("SetMaterial: {e}")))?;
            }
            Ok(())
        }),
    )?;
    put(
        table,
        "GetMaterial",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let collider = scene.world.collider(id.read());
            Ok(collider.and_then(|c| c.material_asset.clone()))
        }),
    )
}

/// `CreateMaterialAsset(path [, fields])`, `GetMaterialAsset(path)` → a field
/// table or nil, `SetMaterialAsset(path, fields)`.
fn register_asset(lua: &Lua, table: &Table) -> Reg {
    put(
        table,
        "CreateMaterialAsset",
        lua.create_function(|_, (path, fields): (String, Option<Table>)| {
            if std::path::Path::new(&path).exists() {
                return Err(err(format!("CreateMaterialAsset: {path:?} already exists")));
            }
            let material = match fields {
                Some(t) => merged(PhysicsMaterial::default(), &t, "CreateMaterialAsset")?,
                None => PhysicsMaterial::default(),
            };
            physics_material::write(&path, material)
                .map(|_| ())
                .map_err(|e| err(format!("CreateMaterialAsset: {e}")))
        }),
    )?;
    put(
        table,
        "GetMaterialAsset",
        lua.create_function(|lua, path: String| {
            physics_material::lookup(&path)
                .map(|m| fields_table(lua, &m))
                .transpose()
        }),
    )?;
    put(
        table,
        "SetMaterialAsset",
        lua.create_function(|_, (path, fields): (String, Table)| {
            let Some(current) = physics_material::lookup(&path) else {
                return Err(err(format!(
                    "SetMaterialAsset: no physics material asset loads from {path:?}"
                )));
            };
            let material = merged(current, &fields, "SetMaterialAsset")?;
            physics_material::write(&path, material)
                .map(|_| ())
                .map_err(|e| err(format!("SetMaterialAsset: {e}")))
        }),
    )
}

/// `m` with the fields `t` names replaced; an error on a non-finite coefficient or
/// an unknown combine mode.
fn merged(mut m: PhysicsMaterial, t: &Table, verb: &str) -> mlua::Result<PhysicsMaterial> {
    for (key, slot) in [
        ("friction", &mut m.friction),
        ("bounciness", &mut m.bounciness),
    ] {
        if let Some(v) = t.get::<Option<f32>>(key)? {
            if !v.is_finite() {
                return Err(err(format!("{verb}: {key} must be finite, got {v}")));
            }
            *slot = v;
        }
    }
    if let Some(name) = t.get::<Option<String>>("friction_combine")? {
        m.friction_combine = combine_mode(Some(name), verb)?;
    }
    if let Some(name) = t.get::<Option<String>>("bounce_combine")? {
        m.bounce_combine = combine_mode(Some(name), verb)?;
    }
    Ok(m)
}

fn fields_table(lua: &Lua, m: &PhysicsMaterial) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("friction", m.friction)?;
    t.set("bounciness", m.bounciness)?;
    t.set("friction_combine", m.friction_combine.name())?;
    t.set("bounce_combine", m.bounce_combine.name())?;
    Ok(t)
}
