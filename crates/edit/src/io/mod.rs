//! crates/edit/src/io/mod.rs — Scene file I/O: save, load, seeding
//!
//! save_to_file / load_from_file. Single active scene: load REPLACES the current
//! World (no multi-scene). Reading a document without a live scene, the extension
//! and the bake files' record are the base's (`rusty_scene::io`).
//!
//! Editor integration:
//!   - the current scene file (`EditorUi::current_scene_path`, one value with the
//!     script runtime's `Scene.Save()` target, — Save writes back HERE
//!     (not a hardcoded demo path); Load (incl. the scene asset's Load Scene button)
//!     sets it.
//!   - one standardised scene extension (`.scene`, JSON inside).
//!
//! Default scene (a deleted one stays deleted,:
//!   - Built in Rust by `edit::default_scene::build`.
//!   - Seeded into the project's `assets/scenes/` on boot with the bundled scripts
//!     (`seed`, through the seed `manifest` so unedited files follow engine fixes,
//!; the seeds are embedded in the binary, `bundled`,.
//!
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
        data.selected_entity_id = None;
        serde_json::to_string(&data).ok()
    };
    let live = compact(saved_data(scene));
    let disk = parse_scene_text(&on_disk, path).ok().and_then(compact);
    live != disk
}

pub fn load_from_file(scene: &mut Scene, path: &str) -> Result<(), LoadError> {
    let json = read_text(path)?;
    let data = parse_scene_text(&json, path)?;
    load_parsed(scene, path, json, data)
}

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
    scene.record_disk_text(path, json);
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
