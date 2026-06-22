//! src/api/scene_prefab.rs — the `Scene` namespace's prefab + asset-spawn verbs.
//!
//! Split out of `api::scene` (size-cap) but registered onto the SAME `Scene` table,
//! so callers still write `Scene.Instantiate(...)`. Covers prefabs v1 (#215, the
//! unpacked copy + `SavePrefab`), the asset spawn (#182), and prefabs v2 (#216, the
//! linked instance + apply / revert / reimport / list-overrides).
//!
//! `Instantiate(prefabPath, ...)` is **linked by default** (the Unity default: an
//! instance that tracks its source). The v1 unpacked copy is the explicit special
//! case `InstantiateUnpacked(prefabPath, ...)`. Every verb routes through
//! `scene::authoring`, so the API and the editor never drift.

use std::cell::RefCell;

use super::{put, Reg};
use crate::scene::authoring;
use crate::scene::Scene;

/// Register every prefab/asset verb onto the (already-created) `Scene` `table`.
pub fn register<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    register_save_and_spawn(scope, table, scene)?;
    register_overrides(scope, table, scene)
}

/// `SavePrefab` + the two spawn verbs (`Instantiate` linked, `InstantiateUnpacked`).
fn register_save_and_spawn<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SavePrefab",
        scope.create_function(|_, (root_id, path): (u32, String)| {
            crate::scene::save_prefab(&scene.borrow(), root_id, &path)
                .map(|()| path)
                .map_err(mlua::Error::RuntimeError)
        }),
    )?;

    put(
        table,
        "Instantiate",
        scope.create_function(|_, (path, rest): (String, mlua::Variadic<mlua::Value>)| {
            instantiate(scene, &path, &rest, true)
        }),
    )?;

    put(
        table,
        "InstantiateUnpacked",
        scope.create_function(|_, (path, rest): (String, mlua::Variadic<mlua::Value>)| {
            instantiate(scene, &path, &rest, false)
        }),
    )
}

/// `Instantiate` / `InstantiateUnpacked` body: dispatch on the path kind. A `.prefab`
/// path stamps a linked (or unpacked) instance; any other path is an asset reference
/// spawned at a position (#182), which has no linked/unpacked distinction.
fn instantiate(
    scene: &RefCell<Scene>,
    path: &str,
    rest: &[mlua::Value],
    linked: bool,
) -> mlua::Result<u32> {
    let mut s = scene.borrow_mut();
    if crate::scene::is_prefab_path(path) {
        let parent = prefab_parent(rest)?;
        let spawn = if linked {
            crate::scene::load_and_instantiate_linked
        } else {
            crate::scene::load_and_instantiate
        };
        spawn(&mut s, path, parent).map_err(mlua::Error::RuntimeError)
    } else {
        let (name, position) = asset_args(rest)?;
        authoring::instantiate_asset(&mut s, path, name.as_deref(), position)
            .map_err(mlua::Error::RuntimeError)
    }
}

/// The linked-instance verbs (#216): apply (record overrides), revert, reimport, and
/// the list-overrides read verb. All take the instance ROOT id.
fn register_overrides<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "ApplyPrefabChanges",
        scope.create_function(|_, root_id: u32| {
            authoring::record_instance_overrides(&mut scene.borrow_mut(), root_id)
                .map_err(mlua::Error::RuntimeError)
        }),
    )?;

    put(
        table,
        "RevertPrefabOverrides",
        scope.create_function(|_, root_id: u32| {
            authoring::revert_instance_overrides(&mut scene.borrow_mut(), root_id)
                .map_err(mlua::Error::RuntimeError)
        }),
    )?;

    put(
        table,
        "ReimportPrefab",
        scope.create_function(|_, root_id: u32| {
            authoring::reimport_instance(&mut scene.borrow_mut(), root_id)
                .map_err(mlua::Error::RuntimeError)
        }),
    )?;

    put(
        table,
        "ListPrefabOverrides",
        scope.create_function(|_, root_id: u32| {
            authoring::list_instance_overrides(&scene.borrow(), root_id)
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

/// Parse the optional `[name][, x, y, z]` trailing args of the asset spawn form. A
/// leading string is the entity name; the next three numbers are the position
/// (defaulting to the origin when omitted).
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
