//! stored-format: the scene file on disk as saved and loaded, and the default scene's seeding

pub mod bundled;
mod lighting_block;
mod load_error;
mod manifest;
mod seed;

pub use lighting_block::{save_lighting, LightingSave};
pub use load_error::LoadError;
pub use manifest::{SeedManifest, SeedOutcome, SEED_MANIFEST_PATH};
pub use seed::{
    build_default_scene, print_refresh, seed_default_scene, seed_default_scripts, seed_project,
    seed_workspace, DEFAULT_SCENE_PATH, DEFAULT_SCRIPTS_DEST_DIR,
};

use crate::serialize::{apply_rehydrated, to_scene_data};
use rusty_scene::io::{parse_scene_text, read_text};
use rusty_scene::serialize::{RehydratedScene, SceneData};
use rusty_scene::Scene;

pub fn save_to_file(scene: &Scene, path: &str) -> Result<(), String> {
    if scene.is_playing() {
        return Err(SAVE_IN_PLAY.to_string());
    }
    let json = scene_json(scene)?;
    // lint: allow(asset-write) a scene, prefab or lighting document, read whole on load
    rusty_core::fs::write_atomic(std::path::Path::new(path), json.as_bytes())
        .map_err(|e| format!("Failed to write scene file: {}", e))?;
    scene.record_disk_text(path, json);
    rusty_scene::lighting::io::save_lighting_sidecar(scene, path)?;
    rusty_scene::nav_settings::save_navmesh(scene, path).map(|_| ())
}

/// Why [`save_to_file`] refused: the World holds the play state, not the scene.
pub const SAVE_IN_PLAY: &str = "a scene cannot be saved during Play: stop Play first";

pub const LIGHTING_IN_PLAY: &str = "lighting cannot be generated during Play: stop Play first";

pub(crate) fn scene_json(scene: &Scene) -> Result<String, String> {
    serde_json::to_string_pretty(&saved_data(scene))
        .map_err(|e| format!("Failed to serialize scene: {}", e))
}

fn saved_data(scene: &Scene) -> SceneData {
    let mut data = to_scene_data(scene);
    if let Ok(root) = std::env::current_dir() {
        let scripts = data.entities.iter_mut().flat_map(|e| e.scripts.iter_mut());
        for script in scripts {
            script.path = rusty_core::paths::relativize(&script.path, &root);
        }
    }
    data
}

pub fn has_unsaved_edits(scene: &Scene, path: &str) -> bool {
    let Some(on_disk) = scene.disk_text_at(path) else {
        return true;
    };
    let compact = |mut data: SceneData| {
        // The editor copies its selection into the scene every frame: not an edit.
        data.selected_entity_id = None;
        serde_json::to_string(&data).ok()
    };
    let live = compact(saved_data(scene));
    // Through the load's drop: a file holding dead lightmap entries opens unedited.
    let held = |mut data: SceneData| {
        crate::serialize::retain_held_lightmaps(&mut data);
        data
    };
    let disk = parse_scene_text(&on_disk, path)
        .ok()
        .map(held)
        .and_then(compact);
    live != disk
}

pub fn load_from_file(scene: &mut Scene, path: &str) -> Result<(), LoadError> {
    let json = read_text(path)?;
    let data = parse_scene_text(&json, path)?;
    load_parsed(scene, path, json, data)
}

/// For a caller that already parsed the text (the editor's follow). Fails only with
/// [`LoadError::Sidecar`], after the World was replaced.
pub fn load_parsed(
    scene: &mut Scene,
    path: &str,
    json: String,
    data: SceneData,
) -> Result<(), LoadError> {
    load_rehydrated(scene, path, json, RehydratedScene::new(data))
}

pub fn load_rehydrated(
    scene: &mut Scene,
    path: &str,
    json: String,
    document: RehydratedScene,
) -> Result<(), LoadError> {
    apply_rehydrated(scene, document);
    scene.renew_id();
    // Recorded even if the sidecar fails: without it the next Generate Lighting or `touch`
    // makes the follow reload over unsaved edits.
    scene.record_disk_text(path, json);
    // Forgotten first: the navmesh and sidecar loads below record this scene's bake files.
    scene.forget_bake_files();
    rusty_scene::nav_settings::load_navmesh(scene, path);
    rusty_scene::lighting::io::load_lighting_sidecar(scene, path).map_err(LoadError::Sidecar)
}

#[cfg(test)]
#[path = "disk_text_tests.rs"]
mod disk_text_tests;
#[cfg(test)]
#[path = "save_tests.rs"]
mod save_tests;
