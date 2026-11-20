//! crates/edit/src/io/mod.rs — Scene file I/O: save, load, seeding
//!
//! save_to_file / load_from_file. Single active scene: load REPLACES the current
//! World (no multi-scene). Reading a document without a live scene, the extension
//! and the bake files' record are the base's (`rusty_scene::io`).
//!
//! Editor integration:
//!   - the current scene file (`EditorUi::current_scene_path`, one value with the
//!     script runtime's `Scene.Save()` target, #1366) — Save writes back HERE
//!     (not a hardcoded demo path); Load (incl. the scene asset's Load Scene button)
//!     sets it.
//!   - one standardised scene extension (`.scene`, JSON inside).
//!
//! Default scene (a deleted one stays deleted, #1618):
//!   - Built in Rust by `edit::default_scene::build` (#667).
//!   - Seeded into the project's `assets/scenes/` on boot with the bundled scripts
//!     (`seed`, through the seed `manifest` so unedited files follow engine fixes,
//!     #746; the seeds are embedded in the binary, `bundled`, #829).
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

/// Serialize the live scene's component VALUES to `path` (pretty JSON, no GPU
/// buffers). Single active scene: this is the only persisted document.
///
/// Refused during Play, as Unity refuses it: the live World then holds the play
/// state (moved enemies, spawned bullets), never the authored scene (#1340). The
/// scene file has two writers, and both refuse: this one (the editor's File menu,
/// `Scene.Save`) and [`save_lighting`] (Generate Lighting, `Lighting.Generate`).
pub fn save_to_file(scene: &Scene, path: &str) -> Result<(), String> {
    if scene.is_playing() {
        return Err(SAVE_IN_PLAY.to_string());
    }
    let json = scene_json(scene)?;
    // lint: allow(asset-write) a scene, prefab or lighting document, read whole on load
    rusty_core::fs::write_atomic(std::path::Path::new(path), json.as_bytes())
        .map_err(|e| format!("Failed to write scene file: {}", e))?;
    scene.record_disk_text(path, json);
    // Heavy baked SH lives beside the scene in `<scene>.lighting.json`, not inline in
    // the human-diffable scene doc (#240): written with the document on every save
    // (removed when no probe is baked). The PVS beside it follows the bake files'
    // rule (`BakeFiles`, #1982).
    rusty_scene::lighting::io::save_lighting_sidecar(scene, path)?;
    // The saved navmesh (#1887), when the scene baked one since it last read or
    // wrote the file (`BakeFiles`, #1982): a save never bakes it.
    rusty_scene::nav_settings::save_navmesh(scene, path).map(|_| ())
}

/// Why [`save_to_file`] refused: the World holds the play state, not the scene.
pub const SAVE_IN_PLAY: &str = "a scene cannot be saved during Play: stop Play first";

/// Why Generate Lighting refused: it writes the scene file, as a save does (#1340).
pub const LIGHTING_IN_PLAY: &str = "lighting cannot be generated during Play: stop Play first";

/// The scene document exactly as [`save_to_file`] writes it, so a seeded default and
/// a saved one compare byte for byte. A script attached by absolute path inside the
/// workspace (the working directory) is written relative to it, `/`-separated, so
/// the file never embeds this machine's paths (#783).
pub(crate) fn scene_json(scene: &Scene) -> Result<String, String> {
    serde_json::to_string_pretty(&saved_data(scene))
        .map_err(|e| format!("Failed to serialize scene: {}", e))
}

/// The document [`scene_json`] writes, before it is text: the scripts' paths made
/// relative to the workspace.
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

/// Whether the live scene has edits the document last read or written at `path`
/// lacks. Computed, never a flag: both sides are compared as compact `SceneData`
/// text without the selection, which the editor copies into the scene every
/// frame and is not an edit. The live side is serialised once, never parsed back:
/// a document's text reads back to the value it was written from. No record for
/// `path` counts as unsaved.
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

/// Load `path` into `scene`, REPLACING the current World (single active scene).
/// Meshes are rehydrated from their `primitive_type` on the way in.
/// A load is a new scene, so it earns a new runtime identity (#355).
pub fn load_from_file(scene: &mut Scene, path: &str) -> Result<(), LoadError> {
    let json = read_text(path)?;
    let data = parse_scene_text(&json, path)?;
    load_parsed(scene, path, json, data)
}

/// The second half of [`load_from_file`], for a caller that already read and parsed
/// the file (the editor's follow, which must know the text parses before it looks
/// at the live scene, #1374): replace the World with `data`, parsed from `json`.
/// Fails only with [`LoadError::Sidecar`], after the World was replaced.
pub fn load_parsed(
    scene: &mut Scene,
    path: &str,
    json: String,
    data: SceneData,
) -> Result<(), LoadError> {
    load_rehydrated(scene, path, json, RehydratedScene::new(data))
}

/// [`load_parsed`] for a document already rehydrated (its meshes built, its models
/// imported through the import cache): a caller that times the import apart from
/// the rest of the load (`make bench-scenes`' boot load).
pub fn load_rehydrated(
    scene: &mut Scene,
    path: &str,
    json: String,
    document: RehydratedScene,
) -> Result<(), LoadError> {
    apply_rehydrated(scene, document);
    scene.renew_id();
    // The World is now this document, so it is "what the engine read" whether or not
    // the sidecar loads: a sidecar changing alone never triggers the follow, so
    // withholding the record would buy no retry, only a reload that discards
    // unsaved edits on the next Generate Lighting, `touch` or rescan (#1374).
    scene.record_disk_text(path, json);
    // A new scene: what the old one saw at its bake files is no record of this one.
    scene.forget_bake_files();
    rusty_scene::nav_settings::load_navmesh(scene, path);
    // Merge the baked SH back onto the just-loaded probe positions (#240). A missing
    // sidecar is fine — probes load with zero SH (positions placed, never baked).
    rusty_scene::lighting::io::load_lighting_sidecar(scene, path).map_err(LoadError::Sidecar)
}

#[cfg(test)]
#[path = "disk_text_tests.rs"]
mod disk_text_tests;
#[cfg(test)]
#[path = "save_tests.rs"]
mod save_tests;
