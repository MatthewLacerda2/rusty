//! src/api/scene_load.rs — the `Scene` namespace's scene-loading verbs (#432).
//!
//! Registered onto the SAME `Scene` table as `api::scene`, so callers write
//! `Scene.Load(path)`. Unity's `SceneManager.LoadScene` / `DontDestroyOnLoad`:
//!
//!   - `Load(path)` — in edit mode, the API face of File ▸ Open: the scene loads at
//!     once and becomes the current scene file. During play the swap is DEFERRED to
//!     the tick's tail (the scene-load phase after the destroy phase), so it never
//!     happens mid-dispatch and replays stay deterministic.
//!   - `New()` — the API face of File ▸ New (#2000): an empty scene with no current
//!     file, through the editor's own `scene::new_scene`. Edit mode only.
//!   - `GetActivePath()` — the current scene file, `nil` when there is none.
//!   - `DontDestroyOnLoad(id)` — the entity and its children survive play-mode loads.
//!   - `LoadAsync(path)` and its operation's verbs — `api::scene_load_async` (#2123).

use std::path::Path;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::scene::Scene;

/// Register the scene-loading verbs onto the (already-created) `Scene` `table`.
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

/// `New()`: File ▸ New's scene half. An attached editor follows the new identity
/// as it follows a scripted `Load` (selection and dirty mark drop), and the path
/// cell is the editor's own (#1366).
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
            let done = crate::scene::new_scene(&mut scene.borrow_mut(), &mut path.borrow_mut());
            done.map_err(|why| runtime(format!("Scene.New: {why}")))
        }),
    )
}

/// Error, naming the verb, when there is no file at `path`.
pub(super) fn check_file(verb: &str, path: &str) -> mlua::Result<()> {
    if Path::new(path).is_file() {
        return Ok(());
    }
    Err(runtime(format!("Scene.{verb}: no scene file at '{path}'")))
}

/// Edit mode's load: File ▸ Open, at once; `path` becomes the current scene file.
/// A sidecar failure comes after the World became the new file's, so the current
/// scene file follows it and the load returns a warning (#1374). Its errors name
/// `verb`, as [`check_file`]'s do.
pub(super) fn load_now(
    verb: &str,
    scene: &std::cell::RefCell<Scene>,
    scene_path: &std::cell::RefCell<Option<String>>,
    path: String,
) -> mlua::Result<Option<String>> {
    let warning = match scene.borrow_mut().load_from_file(&path) {
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
