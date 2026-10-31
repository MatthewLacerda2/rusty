//! src/scene/io/mod.rs — Scene file I/O
//!
//! save_to_file / load_from_file plus path handling. Single active scene: load
//! REPLACES the current World (no multi-scene).
//!
//! Editor integration:
//!   - the current scene file (`EditorUi::current_scene_path`, one value with the
//!     script runtime's `Scene.Save()` target, #1366) — Save writes back HERE
//!     (not a hardcoded demo path); Load (incl. the scene asset's Load Scene button)
//!     sets it.
//!   - one standardised scene extension (`.scene`, JSON inside).
//!
//! Default scene (a deleted one stays deleted, #1618):
//!   - Built in Rust by `scene::default_scene::build` (#667).
//!   - Seeded into the project's `assets/scenes/` on boot with the bundled scripts
//!     (`seed`, through the seed `manifest` so unedited files follow engine fixes,
//!     #746; the seeds are embedded in the binary, `bundled`, #829).

pub mod bundled;
mod disk_text;
mod lighting_block;
mod load_error;
mod manifest;
mod seed;

pub use disk_text::DiskText;
pub use lighting_block::{save_lighting, LightingSave};
pub use load_error::LoadError;
pub use manifest::{SeedManifest, SeedOutcome, SEED_MANIFEST_PATH};
pub use seed::{
    build_default_scene, seed_default_scene, seed_default_scripts, seed_workspace,
    DEFAULT_SCENE_PATH, DEFAULT_SCRIPTS_DEST_DIR,
};

use crate::scene::serialize::{apply_scene_data, to_scene_data, SceneData};
use crate::scene::Scene;

/// The one standardised scene file extension (JSON inside).
pub const SCENE_EXTENSION: &str = "scene";

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
    std::fs::write(path, &json).map_err(|e| format!("Failed to write scene file: {}", e))?;
    scene.disk_text.record(path, json);
    // Heavy baked SH lives beside the scene in `<scene>.lighting.json`, not inline in
    // the human-diffable scene doc (#240). Written only when probes are baked.
    crate::scene::lighting::io::save_lighting_sidecar(scene, path)?;
    Ok(())
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
    let mut data = to_scene_data(scene);
    if let Ok(root) = std::env::current_dir() {
        let scripts = data.entities.iter_mut().flat_map(|e| e.scripts.iter_mut());
        for script in scripts {
            script.path = crate::core::paths::relativize(&script.path, &root);
        }
    }
    serde_json::to_string_pretty(&data).map_err(|e| format!("Failed to serialize scene: {}", e))
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
    apply_scene_data(scene, data);
    scene.renew_id();
    // The World is now this document, so it is "what the engine read" whether or not
    // the sidecar loads: a sidecar changing alone never triggers the follow, so
    // withholding the record would buy no retry, only a reload that discards
    // unsaved edits on the next Generate Lighting, `touch` or rescan (#1374).
    scene.disk_text.record(path, json);
    // Merge the baked SH back onto the just-loaded probe positions (#240). A missing
    // sidecar is fine — probes load with zero SH (positions placed, never baked).
    crate::scene::lighting::io::load_lighting_sidecar(scene, path).map_err(LoadError::Sidecar)
}

/// Read and parse the scene document at `path` without touching any live scene — so
/// a caller can fail cleanly before tearing the current one down (#432).
pub fn read_scene_file(path: &str) -> Result<SceneData, String> {
    parse_scene_text(&read_text(path)?, path)
}

fn read_text(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("Failed to read scene file {}: {}", path, e))
}

/// Parse `json`, the scene document read from `path` (named in the error).
pub fn parse_scene_text(json: &str, path: &str) -> Result<SceneData, String> {
    serde_json::from_str(json).map_err(|e| format!("Failed to deserialize scene {}: {}", path, e))
}

#[cfg(test)]
#[path = "save_tests.rs"]
mod save_tests;
