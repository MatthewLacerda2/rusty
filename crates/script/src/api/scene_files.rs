//! The scene files a World is authored from (Unity's multi-scene editing): `Save` writes each
//! loaded scene to its own file, `New(path)` adds an empty one, `MoveTo` moves a root between
//! them, and an `"Additive"` `Load` in edit mode joins a file at once.
use super::scene_load::runtime;
use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_scene::Scene;
use std::cell::RefCell;

pub fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    scene_path: ApiCell<Option<String>>,
) -> Reg {
    put(
        table,
        "Save",
        lua.create_function(move |lua, path: Option<String>| {
            let (scene, active) = (scene.get(lua)?, scene_path.get(lua)?);
            let saved = save(&scene, &active, path);
            saved.map_err(|e| runtime(format!("Scene.Save: {e}")))
        }),
    )?;
    put(
        table,
        "New",
        lua.create_function(move |lua, path: Option<String>| {
            let (scene, active) = (scene.get(lua)?, scene_path.get(lua)?);
            let Some(path) = path else {
                let mut scene = scene.borrow_mut();
                let done = rusty_scene::new_scene(&mut scene, &mut active.borrow_mut());
                return done.map_err(|why| runtime(format!("Scene.New: {why}")));
            };
            add_empty(&scene, &active, path).map_err(|e| runtime(format!("Scene.New: {e}")))
        }),
    )?;
    put(
        table,
        "MoveTo",
        lua.create_function(move |lua, (id, path): (EntityArg, String)| {
            let id = id.live("Scene", "MoveTo")?;
            let scene = scene.get(lua)?;
            let mut scene = scene.borrow_mut();
            let handle = scene.loaded.handle_of(&path).ok_or_else(|| {
                let loaded: Vec<String> = scene.loaded.paths(None).into_iter().flatten().collect();
                runtime(format!(
                    "Scene.MoveTo: '{path}' is not a loaded scene (loaded: {loaded:?}); with one \
                     scene loaded there is nowhere to move to"
                ))
            })?;
            let moved = scene.move_to_scene(id, handle);
            moved.map_err(|why| runtime(format!("Scene.MoveTo: {why}")))
        }),
    )
}

/// The written path: `path`, or with no path the active scene's (every loaded one is written).
fn save(
    scene: &RefCell<Scene>,
    active: &RefCell<Option<String>>,
    path: Option<String>,
) -> Result<String, String> {
    let several = scene.borrow().loaded.active_of_several().is_some();
    match (path, several) {
        (None, _) => {
            let current = active.borrow().clone();
            rusty_edit::scenes::save_all(&scene.borrow(), current.as_deref())?;
            current.ok_or_else(|| "the active scene has no file yet".to_string())
        }
        (Some(path), false) => {
            rusty_edit::save_to_file(&scene.borrow(), &path)?;
            *active.borrow_mut() = Some(path.clone());
            Ok(path)
        }
        (Some(path), true) => {
            let mut scene = scene.borrow_mut();
            let renames_active = scene.loaded.handle_of(&path).is_none();
            rusty_edit::scenes::save_as(&mut scene, &path)?;
            if renames_active {
                *active.borrow_mut() = Some(path.clone());
            }
            Ok(path)
        }
    }
}

fn add_empty(
    scene: &RefCell<Scene>,
    active: &RefCell<Option<String>>,
    path: String,
) -> Result<(), String> {
    let mut scene = scene.borrow_mut();
    if scene.is_playing() {
        return Err("an empty scene is added in edit mode only".to_string());
    }
    if scene.loaded.handle_of(&path).is_some() || active.borrow().as_deref() == Some(&path) {
        return Err(format!("'{path}' is already loaded"));
    }
    if std::path::Path::new(&path).exists() {
        return Err(format!(
            "a file is already at '{path}': load it with Scene.Load(path, \"Additive\")"
        ));
    }
    let base = active.borrow().clone();
    rusty_edit::scenes::add_empty(&mut scene, base, path);
    Ok(())
}

/// `Scene.Load(path, "Additive")` in edit mode: the file joins now, no script runs.
pub(super) fn load_additive_now(
    scene: &RefCell<Scene>,
    active: &RefCell<Option<String>>,
    path: String,
) -> mlua::Result<Option<String>> {
    let base = active.borrow().clone();
    let joined = rusty_edit::scenes::load_additive(&mut scene.borrow_mut(), base, &path);
    let joined = joined.map_err(|e| runtime(format!("Scene.Load: {e}")))?;
    let mut warnings = Vec::new();
    if joined.disagrees {
        warnings.push(
            "its layers or collision matrix differ from the first scene's, whose are kept"
                .to_string(),
        );
    }
    if !joined.skipped.is_empty() {
        warnings.push(format!(
            "an additive scene's bakes are not applied yet, so it has no {}",
            joined.skipped.join(", ")
        ));
    }
    Ok((!warnings.is_empty()).then(|| format!("Scene.Load: {path}: {}", warnings.join("; "))))
}
