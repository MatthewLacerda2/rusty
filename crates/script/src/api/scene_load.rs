use std::path::Path;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_scene::Scene;

pub fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    scene_path: ApiCell<Option<String>>,
) -> Reg {
    put(
        table,
        "Load",
        lua.create_function(move |lua, path: String| {
            let scene = &*scene.get(lua)?;
            check_file("Load", &path)?;
            if scene.borrow().is_playing() {
                scene.borrow_mut().request_load(path);
                return Ok(None);
            }
            load_now("Load", scene, &*scene_path.get(lua)?, path)
        }),
    )?;
    super::scene_load_async::register(lua, table, scene, scene_path)?;

    register_new(lua, table, scene, scene_path)?;

    put(
        table,
        "GetActivePath",
        lua.create_function(move |lua, ()| Ok(scene_path.get(lua)?.borrow().clone())),
    )?;

    // Only play has a load to survive; Unity refuses the call in edit mode too.
    put(
        table,
        "DontDestroyOnLoad",
        lua.create_function(move |lua, id: EntityArg| {
            scene.with(lua, |scene| {
                let id = id.live("Scene", "DontDestroyOnLoad")?;
                if !scene.borrow().is_playing() {
                    return Err(runtime(
                        "Scene.DontDestroyOnLoad only works during play".to_string(),
                    ));
                }
                Ok(scene.borrow_mut().dont_destroy_on_load(id))
            })
        }),
    )
}

fn register_new(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    scene_path: ApiCell<Option<String>>,
) -> Reg {
    put(
        table,
        "New",
        lua.create_function(move |lua, ()| {
            let scene = &*scene.get(lua)?;
            let path = &*scene_path.get(lua)?;
            let done = rusty_scene::new_scene(&mut scene.borrow_mut(), &mut path.borrow_mut());
            done.map_err(|why| runtime(format!("Scene.New: {why}")))
        }),
    )
}

pub(super) fn check_file(verb: &str, path: &str) -> mlua::Result<()> {
    if Path::new(path).is_file() {
        return Ok(());
    }
    Err(runtime(format!("Scene.{verb}: no scene file at '{path}'")))
}

pub(super) fn load_now(
    verb: &str,
    scene: &std::cell::RefCell<Scene>,
    scene_path: &std::cell::RefCell<Option<String>>,
    path: String,
) -> mlua::Result<Option<String>> {
    let warning = match rusty_edit::load_from_file(&mut scene.borrow_mut(), &path) {
        Ok(()) => None,
        Err(e) if e.replaced_world() => Some(format!("Scene.{verb}: {path}: {e}")),
        Err(e) => return Err(runtime(format!("Scene.{verb}: {e}"))),
    };
    *scene_path.borrow_mut() = Some(path);
    Ok(warning)
}

pub(super) fn runtime(message: String) -> mlua::Error {
    mlua::Error::RuntimeError(message)
}
