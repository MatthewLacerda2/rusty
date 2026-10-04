//! src/api/scene_load.rs — the `Scene` namespace's scene-loading verbs (#432).
//!
//! Registered onto the SAME `Scene` table as `api::scene`, so callers write
//! `Scene.Load(path)`. Unity's `SceneManager.LoadScene` / `DontDestroyOnLoad`:
//!
//!   - `Load(path)` — in edit mode, the API face of File ▸ Open: the scene loads at
//!     once and becomes the current scene file. During play the swap is DEFERRED to
//!     the tick's tail (the scene-load phase after the destroy phase), so it never
//!     happens mid-dispatch and replays stay deterministic.
//!   - `GetActivePath()` — the current scene file, `nil` when there is none.
//!   - `DontDestroyOnLoad(id)` — the entity and its children survive play-mode loads.

use std::cell::RefCell;
use std::path::Path;

use super::{put, Reg};
use crate::scene::Scene;

/// Register the scene-loading verbs onto the (already-created) `Scene` `table`.
pub fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    scene_path: &'scope RefCell<Option<String>>,
    is_playing: &'scope RefCell<bool>,
) -> Reg {
    put(
        table,
        "Load",
        scope.create_function(move |_, path: String| {
            if !Path::new(&path).is_file() {
                return Err(runtime(format!("Scene.Load: no scene file at '{path}'")));
            }
            if *is_playing.borrow() {
                scene.borrow_mut().request_load(path);
            } else {
                scene
                    .borrow_mut()
                    .load_from_file(&path)
                    .map_err(|e| runtime(format!("Scene.Load: {e}")))?;
                *scene_path.borrow_mut() = Some(path);
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "GetActivePath",
        scope.create_function(move |_, ()| Ok(scene_path.borrow().clone())),
    )?;

    // Only play has a load to survive; Unity refuses the call in edit mode too.
    put(
        table,
        "DontDestroyOnLoad",
        scope.create_function(move |_, id: u32| {
            if !*is_playing.borrow() {
                return Err(runtime(
                    "Scene.DontDestroyOnLoad only works during play".to_string(),
                ));
            }
            Ok(scene.borrow_mut().dont_destroy_on_load(id))
        }),
    )
}

fn runtime(message: String) -> mlua::Error {
    mlua::Error::RuntimeError(message)
}
