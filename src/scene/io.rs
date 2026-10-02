//! src/scene/io.rs — Scene file I/O
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
//!   - Seeded into  project/scenes/  on boot, the same way bot.lua is seeded,
//!     because /project/ is the gitignored runtime workspace.

use std::path::Path;

use crate::scene::serialize::{apply_scene_data, to_scene_data, SceneData};
use crate::scene::Scene;

/// The one standardised scene file extension (JSON inside).
pub const SCENE_EXTENSION: &str = "scene";

/// Where the default scene is seeded into the gitignored project workspace.
pub const DEFAULT_SCENE_PATH: &str = "project/scenes/default.scene";

/// Tracked authoritative copies of the bundled default scripts (the player
/// controller + the enemy brain) that ship WITH the engine.
pub const DEFAULT_SCRIPTS_SOURCE_DIR: &str = "assets/scripts";
/// Where the bundled scripts are seeded into the gitignored project workspace, so
/// scenes referencing `project/assets/scripts/<name>.lua` resolve on boot.
pub const DEFAULT_SCRIPTS_DEST_DIR: &str = "project/assets/scripts";

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
    let data = to_scene_data(scene);
    let json = serde_json::to_string_pretty(&data)
        .map_err(|e| format!("Failed to serialize scene: {}", e))?;
    std::fs::write(path, json).map_err(|e| format!("Failed to write scene file: {}", e))?;
    // Heavy baked SH lives beside the scene in `<scene>.lighting.json`, not inline in
    // the human-diffable scene doc (#240). Written only when probes are baked.
    crate::scene::lighting::io::save_lighting_sidecar(scene, path)?;
    Ok(())
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

/// Seed the default scene into the gitignored project workspace on boot, the same
/// way `bot.lua` is seeded: built by [`default_scene::build`] and saved, only if the
/// target is missing (delete it to get the current default back). Its texture and
/// shader are seeded every time, each only if missing. Returns the seeded path so
/// the caller can load it as the boot scene.
///
/// [`default_scene::build`]: crate::scene::default_scene::build
pub fn seed_default_scene() -> String {
    use crate::scene::default_scene;
    default_scene::seed_default_assets();
    if let Some(parent) = Path::new(DEFAULT_SCENE_PATH).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if !Path::new(DEFAULT_SCENE_PATH).exists() {
        let mut scene = Scene::new();
        default_scene::build(&mut scene, default_scene::BOT_SCRIPT);
        if let Err(e) = save_to_file(&scene, DEFAULT_SCENE_PATH) {
            eprintln!("[Scene] seeding the default scene failed: {e}");
        }
    }
    DEFAULT_SCENE_PATH.to_string()
}

/// Seed the bundled default scripts (`assets/scripts/*.lua`) into the gitignored
/// `project/assets/scripts/` workspace on boot, the same pattern as the scene and
/// `bot.lua`. Existing files are left untouched so local edits survive. Idempotent.
/// The UI widget kit's scripts and prefabs are seeded too — those directories are
/// engine-owned and rewritten (see `authoring::ui_widgets::seed`).
pub fn seed_default_scripts() {
    crate::scene::authoring::ui_widgets::seed();
    std::fs::create_dir_all(DEFAULT_SCRIPTS_DEST_DIR).ok();
    let Ok(entries) = std::fs::read_dir(DEFAULT_SCRIPTS_SOURCE_DIR) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_lua = path.extension().and_then(|e| e.to_str()) == Some("lua");
        if !is_lua {
            continue;
        }
        let Some(name) = path.file_name() else {
            continue;
        };
        let dest = Path::new(DEFAULT_SCRIPTS_DEST_DIR).join(name);
        if !dest.exists() {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                std::fs::write(&dest, contents).ok();
            }
        }
    }
}
