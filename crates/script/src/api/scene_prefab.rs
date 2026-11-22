use std::cell::RefCell;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_edit::authoring;
use rusty_scene::Scene;

pub fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    register_save_and_spawn(lua, table, scene)?;
    register_overrides(lua, table, scene)?;
    register_apply_to_source(lua, table, scene)
}

fn register_save_and_spawn(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SavePrefab",
        lua.create_function(move |lua, (root_id, path): (EntityArg, String)| {
            scene.with(lua, |scene| {
                let root_id = root_id.live("Scene", "SavePrefab")?;
                rusty_edit::save_prefab(&scene.borrow(), root_id, &path)
                    .map(|()| path)
                    .map_err(mlua::Error::RuntimeError)
            })
        }),
    )?;

    put(
        table,
        "Instantiate",
        lua.create_function(
            move |lua, (path, rest): (String, mlua::Variadic<mlua::Value>)| {
                instantiate(&*scene.get(lua)?, &path, &rest, true)
            },
        ),
    )?;

    put(
        table,
        "InstantiateUnpacked",
        lua.create_function(
            move |lua, (path, rest): (String, mlua::Variadic<mlua::Value>)| {
                instantiate(&*scene.get(lua)?, &path, &rest, false)
            },
        ),
    )
}

fn instantiate(
    scene: &RefCell<Scene>,
    path: &str,
    rest: &[mlua::Value],
    linked: bool,
) -> mlua::Result<u32> {
    let mut s = scene.borrow_mut();
    if rusty_edit::is_prefab_path(path) {
        let parent = prefab_parent(rest)?;
        let spawn = if linked {
            rusty_edit::load_and_instantiate_linked
        } else {
            rusty_edit::load_and_instantiate
        };
        spawn(&mut s, path, parent).map_err(mlua::Error::RuntimeError)
    } else {
        let (name, position) = asset_args(rest)?;
        authoring::instantiate_asset(&mut s, path, name.as_deref(), position)
            .map_err(mlua::Error::RuntimeError)
    }
}

fn register_overrides(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "RevertPrefabOverrides",
        lua.create_function(move |lua, root_id: EntityArg| {
            let root_id = root_id.live("Scene", "RevertPrefabOverrides")?;
            authoring::revert_instance_overrides(&mut scene.get(lua)?.borrow_mut(), root_id)
                .map_err(mlua::Error::RuntimeError)
        }),
    )?;

    put(
        table,
        "ReimportPrefab",
        lua.create_function(move |lua, root_id: EntityArg| {
            let root_id = root_id.live("Scene", "ReimportPrefab")?;
            authoring::reimport_instance(&mut scene.get(lua)?.borrow_mut(), root_id)
                .map_err(mlua::Error::RuntimeError)
        }),
    )?;

    put(
        table,
        "ListPrefabOverrides",
        lua.create_function(move |lua, root_id: EntityArg| {
            authoring::list_instance_overrides(&scene.get(lua)?.borrow(), root_id.read())
                .map_err(mlua::Error::RuntimeError)
        }),
    )
}

fn register_apply_to_source(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "ApplyPrefabToSource",
        lua.create_function(move |lua, root_id: EntityArg| {
            let root_id = root_id.live("Scene", "ApplyPrefabToSource")?;
            authoring::apply_instance_to_source(&mut scene.get(lua)?.borrow_mut(), root_id)
                .map_err(mlua::Error::RuntimeError)
        }),
    )?;

    put(
        table,
        "RevertPrefabField",
        lua.create_function(move |lua, (entity_id, path): (EntityArg, String)| {
            let entity_id = entity_id.live("Scene", "RevertPrefabField")?;
            authoring::revert_instance_field(&mut scene.get(lua)?.borrow_mut(), entity_id, &path)
                .map_err(mlua::Error::RuntimeError)
        }),
    )?;

    put(
        table,
        "ApplyPrefabFieldToSource",
        lua.create_function(move |lua, (entity_id, path): (EntityArg, String)| {
            let entity_id = entity_id.live("Scene", "ApplyPrefabFieldToSource")?;
            authoring::apply_instance_field_to_source(
                &mut scene.get(lua)?.borrow_mut(),
                entity_id,
                &path,
            )
            .map_err(mlua::Error::RuntimeError)
        }),
    )
}

/// Parse the optional `parentId` trailing arg of the `.prefab` spawn forms.
fn prefab_parent(rest: &[mlua::Value]) -> mlua::Result<Option<u32>> {
    match rest.first() {
        None | Some(mlua::Value::Nil) => Ok(None),
        Some(mlua::Value::Integer(i)) => Ok(Some(*i as u32)),
        Some(mlua::Value::Number(n)) => Ok(Some(*n as u32)),
        Some(_) => Err(mlua::Error::RuntimeError(
            "Scene.Instantiate(prefab): parentId must be a number".to_string(),
        )),
    }
}

fn asset_args(rest: &[mlua::Value]) -> mlua::Result<(Option<String>, glam::Vec3)> {
    let (name, coords) = match rest.first() {
        Some(mlua::Value::String(s)) => (Some(s.to_str()?.to_string()), &rest[1..]),
        _ => (None, rest),
    };
    let f = |i: usize| -> f32 {
        match coords.get(i) {
            Some(mlua::Value::Integer(n)) => *n as f32,
            Some(mlua::Value::Number(n)) => *n as f32,
            _ => 0.0,
        }
    };
    Ok((name, glam::Vec3::new(f(0), f(1), f(2))))
}
