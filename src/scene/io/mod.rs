//! src/scene/io/mod.rs — Scene file I/O
//!
//! save_to_file / load_from_file plus path handling. Single active scene: load
//! REPLACES the current World (no multi-scene).
//!
//! Editor integration:
//!   - `EditorUi.current_scene_path: Option<String>` — Save writes back HERE
//!     (not a hardcoded demo path); Load (incl. double-click in the assets
//!     browser) sets it.
//!   - one standardised scene extension (`.scene`, JSON inside).
//!
//! Default scene (there is always at least one):
//!   - Built in Rust by `scene::default_scene::build` (#667).
//!   - Seeded into the project's `assets/scenes/` on boot with the bundled scripts
//!     (`seed`, through the seed `manifest` so unedited files follow engine fixes,
//!     #746; the seeds are embedded in the binary, `bundled`, #829).

pub mod bundled;
mod manifest;
mod seed;

use std::path::Path;

pub use manifest::{SeedManifest, SeedOutcome, SEED_MANIFEST_PATH};
pub use seed::{
    build_default_scene, seed_default_scene, seed_default_scripts, seed_workspace,
    DEFAULT_SCENE_PATH, DEFAULT_SCRIPTS_DEST_DIR,
};

use crate::scene::serialize::{apply_scene_data, to_scene_data, SceneData};
use crate::scene::Scene;

/// The one standardised scene file extension (JSON inside).
pub const SCENE_EXTENSION: &str = "scene";

/// True if `path` looks like a scene file (`.scene`).
pub fn is_scene_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case(SCENE_EXTENSION))
        .unwrap_or(false)
}

/// Serialize the live scene's component VALUES to `path` (pretty JSON, no GPU
/// buffers). Single active scene: this is the only persisted document.
pub fn save_to_file(scene: &Scene, path: &str) -> Result<(), String> {
    let json = scene_json(scene)?;
    std::fs::write(path, json).map_err(|e| format!("Failed to write scene file: {}", e))?;
    // Heavy baked SH lives beside the scene in `<scene>.lighting.json`, not inline in
    // the human-diffable scene doc (#240). Written only when probes are baked.
    crate::scene::lighting::io::save_lighting_sidecar(scene, path)?;
    Ok(())
}

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
pub fn load_from_file(scene: &mut Scene, path: &str) -> Result<(), String> {
    let data = read_scene_file(path)?;
    apply_scene_data(scene, data);
    scene.renew_id();
    // Merge the baked SH back onto the just-loaded probe positions (#240). A missing
    // sidecar is fine — probes load with zero SH (positions placed, never baked).
    crate::scene::lighting::io::load_lighting_sidecar(scene, path)?;
    Ok(())
}

/// Read and parse the scene document at `path` without touching any live scene — so
/// a caller can fail cleanly before tearing the current one down (#432).
pub fn read_scene_file(path: &str) -> Result<SceneData, String> {
    let json = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read scene file {}: {}", path, e))?;
    serde_json::from_str(&json).map_err(|e| format!("Failed to deserialize scene {}: {}", path, e))
}

#[cfg(test)]
#[path = "save_tests.rs"]
mod save_tests;
