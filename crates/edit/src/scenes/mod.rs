//! stored-format: Unity's multi-scene editing. Each loaded scene saves to its own file.

mod document;
mod reload;

pub use reload::reload_loaded;

use crate::load::additive::{join_prepared, Joined};
use crate::load::prepare::PreparedScene;
use rusty_scene::Scene;

/// Edit mode's additive load: the file at `path` joins now, its text recorded.
pub fn load_additive(
    scene: &mut Scene,
    base: Option<String>,
    path: &str,
) -> Result<Joined, String> {
    let text = rusty_scene::io::read_text(path)?;
    let data = rusty_scene::parse_scene_text(&text, path)?;
    let document = rusty_scene::rehydrate::RehydratedScene::new(data);
    let sidecars = crate::load::prepare::SceneSidecars::read(path);
    let prepared = PreparedScene::new(path.to_string(), document, sidecars);
    let (_, joined) = join_additive(scene, base, prepared);
    scene.record_disk_text_also(path, text);
    Ok(joined)
}

/// Edit mode and Play: before it, the World becomes its first scene, `base`.
pub fn join_additive(
    scene: &mut Scene,
    base: Option<String>,
    prepared: PreparedScene,
) -> (u32, Joined) {
    scene.open_loaded(base);
    let path = prepared.path().to_string();
    let mut joined = join_prepared(scene, prepared);
    let file = std::mem::take(&mut joined.file);
    let settings = joined.settings.clone();
    let handle = scene.join_loaded(None, path, settings, &joined.roots, file);
    (handle, joined)
}

pub fn add_empty(scene: &mut Scene, base: Option<String>, path: String) -> u32 {
    let empty = crate::serialize::to_scene_data(&Scene::new());
    scene.add_empty_scene(base, path, empty)
}

/// One scene's save: the editor's mark clears once no loaded file holds an unsaved edit.
pub(crate) fn save_one(scene: &Scene, handle: u32, path: &str) -> Result<(), String> {
    save_member(scene, handle, path)?;
    let paths = scene.loaded.paths(None).into_iter().flatten();
    if !paths
        .into_iter()
        .any(|p| crate::io::has_unsaved_edits(scene, &p))
    {
        scene.note_saved();
    }
    Ok(())
}

fn save_member(scene: &Scene, handle: u32, path: &str) -> Result<(), String> {
    let doc = document::document(scene, handle).ok_or("that scene is not loaded")?;
    for line in &doc.dropped {
        log::warn!(
            "[Scene] {path}: {line}: not written, as no reference crosses scenes in a saved file"
        );
    }
    let mut data = doc.data;
    crate::io::relative_scripts(&mut data);
    let json = serde_json::to_string_pretty(&data)
        .map_err(|e| format!("Failed to serialize scene: {e}"))?;
    // lint: allow(asset-write) a scene document, read whole on load
    rusty_core::fs::write_atomic(std::path::Path::new(path), json.as_bytes())
        .map_err(|e| format!("Failed to write scene file: {e}"))?;
    scene.record_disk_text_also(path, json);
    let file = scene
        .loaded
        .file(handle)
        .ok_or("that scene is not loaded")?;
    file.keep(&doc.ids, doc.next_written);
    // The World's bakes are the first scene's.
    if file.carried.is_none() {
        rusty_scene::lighting::io::save_lighting_sidecar(scene, path)?;
        rusty_scene::nav_settings::save_navmesh(scene, path)?;
    }
    Ok(())
}

/// Save As: the active scene's file becomes `path`.
pub fn save_as(scene: &mut Scene, path: &str) -> Result<(), String> {
    if scene.is_playing() {
        return Err(crate::io::SAVE_IN_PLAY.to_string());
    }
    let handle = match scene.loaded.handle_of(path) {
        Some(handle) => handle,
        None => scene.loaded.active(),
    };
    let old = scene.loaded.path_of(handle).map(str::to_string);
    save_one(scene, handle, path)?;
    scene.loaded.rename(handle, path);
    if let Some(old) = old.filter(|o| o != path) {
        scene.forget_disk_text_at(&old);
    }
    Ok(())
}

pub(crate) fn member_data(scene: &Scene, path: &str) -> Option<rusty_scene::SceneData> {
    let handle = scene.loaded.handle_of(path)?;
    let mut data = document::document(scene, handle)?.data;
    crate::io::relative_scripts(&mut data);
    Some(data)
}

/// `Scene.Save()` and File ▸ Save; an error names each scene not written.
pub fn save_all(scene: &Scene, current: Option<&str>) -> Result<Vec<String>, String> {
    if scene.loaded.handles().is_empty() {
        let path = current.ok_or("no path given and no current scene file is set")?;
        crate::io::save_to_file(scene, path)?;
        return Ok(vec![path.to_string()]);
    }
    if scene.is_playing() {
        return Err(crate::io::SAVE_IN_PLAY.to_string());
    }
    let mut saved = Vec::new();
    let mut failed = Vec::new();
    for handle in scene.loaded.handles() {
        let Some(path) = scene.loaded.path_of(handle) else {
            failed
                .push("a loaded scene has no file yet: save it with Scene.Save(path)".to_string());
            continue;
        };
        match save_member(scene, handle, path) {
            Ok(()) => saved.push(path.to_string()),
            Err(why) => failed.push(format!("{path}: {why}")),
        }
    }
    if !failed.is_empty() {
        return Err(failed.join("; "));
    }
    scene.note_saved();
    Ok(saved)
}

#[cfg(test)]
mod tests;
