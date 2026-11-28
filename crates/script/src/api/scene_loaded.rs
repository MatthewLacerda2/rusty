//! Unity's `SceneManager` over several loaded scenes: `Unload`, `SetActiveScene`, `GetLoaded`
//! and `Of` (a GameObject's `scene`). The active path is the one `GetActivePath` reads.
use super::scene_load::runtime;
use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_scene::runtime::loads::LoadMode;
use rusty_scene::Scene;

pub fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    scene_path: ApiCell<Option<String>>,
) -> Reg {
    put(
        table,
        "Unload",
        lua.create_function(move |lua, path: String| {
            let (scene, active) = (scene.get(lua)?, scene_path.get(lua)?);
            let handle = loaded_handle(&scene.borrow(), "Unload", &path, &active.borrow())?;
            let unloaded = scene.borrow_mut().unload_scene(handle);
            let now = unloaded.map_err(|why| runtime(format!("Scene.Unload: '{path}': {why}")))?;
            *active.borrow_mut() = now;
            Ok(())
        }),
    )?;
    put(
        table,
        "SetActiveScene",
        lua.create_function(move |lua, path: String| {
            let (scene, active) = (scene.get(lua)?, scene_path.get(lua)?);
            if scene.borrow().active_handle().is_none() && active.borrow().as_deref() == Some(&path)
            {
                return Ok(());
            }
            let handle = loaded_handle(&scene.borrow(), "SetActiveScene", &path, &active.borrow())?;
            let now = scene.borrow_mut().set_active_scene(handle);
            *active.borrow_mut() = now.filter(|p| !p.is_empty());
            Ok(())
        }),
    )?;
    put(
        table,
        "GetLoaded",
        lua.create_function(move |lua, ()| {
            let (scene, active) = (scene.get(lua)?, scene_path.get(lua)?);
            let paths = scene.borrow().loaded.paths(active.borrow().as_deref());
            let paths = paths.into_iter().map(Option::unwrap_or_default);
            lua.create_sequence_from(paths)
        }),
    )?;
    put(
        table,
        "Of",
        lua.create_function(move |lua, id: EntityArg| {
            let (scene, active) = (scene.get(lua)?, scene_path.get(lua)?);
            let Some(id) = id.get() else {
                return Ok(None);
            };
            let path = scene.borrow().member_path(id, active.borrow().as_deref());
            Ok(path)
        }),
    )
}

/// Before any additive load the one scene has no handle: it is loaded, and the last one.
fn loaded_handle(
    scene: &Scene,
    verb: &str,
    path: &str,
    active: &Option<String>,
) -> mlua::Result<u32> {
    let found = scene.loaded.handle_of(path);
    found.ok_or_else(|| {
        let loaded = scene.loaded.paths(active.as_deref());
        let loaded: Vec<String> = loaded.into_iter().flatten().collect();
        runtime(match loaded.iter().any(|p| p == path) {
            true => format!("Scene.{verb}: '{path}': the last loaded scene cannot be unloaded"),
            false => format!(
                "Scene.{verb}: '{path}' is not a loaded scene (loaded: {loaded:?}); pass one of \
                 those paths, as Scene.GetLoaded() lists them"
            ),
        })
    })
}

/// `Scene.Load`'s and `LoadAsync`'s optional mode, Unity's `LoadSceneMode`; additive loads are
/// a play-mode feature, as the editor opens one scene.
pub(super) fn load_mode(verb: &str, mode: Option<String>, playing: bool) -> mlua::Result<LoadMode> {
    let Some(name) = mode else {
        return Ok(LoadMode::Single);
    };
    let mode = LoadMode::parse(&name).ok_or_else(|| {
        runtime(format!(
            "Scene.{verb}: unknown mode '{name}'; expected one of {:?}",
            LoadMode::NAMES
        ))
    })?;
    if mode == LoadMode::Additive && !playing {
        return Err(runtime(format!(
            "Scene.{verb}: an \"Additive\" load only works during play"
        )));
    }
    Ok(mode)
}
