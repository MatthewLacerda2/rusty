//! src/scene/io/seed.rs — seed the engine's defaults into the project workspace.
//!
//! The default scene (built in Rust by `default_scene::build`, #667) and the bundled
//! scripts (`engine/scripts/*.lua`) are seeded into the gitignored `project/` on
//! boot, through the seed manifest (#746): a file nobody edited follows the engine's
//! current default, an edited one is kept.

use std::path::{Path, PathBuf};

use super::manifest::{SeedManifest, SEED_MANIFEST_PATH};
use super::scene_json;
use crate::scene::{default_scene, Scene};

/// Where the default scene is seeded into the gitignored project workspace.
pub const DEFAULT_SCENE_PATH: &str = "project/scenes/default.scene";

/// Tracked authoritative copies of the bundled default scripts (the player
/// controller + the enemy brain) that ship WITH the engine.
pub const DEFAULT_SCRIPTS_SOURCE_DIR: &str = "engine/scripts";
/// Where the bundled scripts are seeded into the gitignored project workspace, so
/// scenes referencing `project/assets/scripts/<name>.lua` resolve on boot.
pub const DEFAULT_SCRIPTS_DEST_DIR: &str = "project/assets/scripts";

/// Build the engine's current default scene into a fresh `scene`, replacing what was
/// there, and return its path. File ▸ Reset Scene goes through here, never through
/// the seeded file, which may be an older default or the user's edit.
pub fn build_default_scene(scene: &mut Scene) -> String {
    default_scene::seed_default_assets();
    *scene = Scene::new();
    default_scene::build(scene, default_scene::BOT_SCRIPT);
    DEFAULT_SCENE_PATH.to_string()
}

/// Seed the default scene into the project workspace on boot, through the seed
/// manifest. Its texture and shader are seeded every time, each only if missing.
/// Returns the seeded path so the caller can load it as the boot scene.
pub fn seed_default_scene() -> String {
    default_scene::seed_default_assets();
    match default_scene_bytes() {
        Ok(json) => {
            let hint = "File ▸ Reset Scene, or delete the file, to take the new one";
            let files = [(Path::new(DEFAULT_SCENE_PATH).to_path_buf(), json)];
            seed_files(Path::new(SEED_MANIFEST_PATH), &files, hint);
        }
        Err(e) => eprintln!("[Scene] seeding the default scene failed: {e}"),
    }
    DEFAULT_SCENE_PATH.to_string()
}

/// Seed the bundled default scripts (`engine/scripts/*.lua`) into
/// `project/assets/scripts/` on boot, through the seed manifest. Idempotent.
/// The UI widget kit's scripts and prefabs are seeded too — those directories are
/// engine-owned and rewritten (see `authoring::ui_widgets::seed`).
pub fn seed_default_scripts() {
    crate::scene::authoring::ui_widgets::seed();
    let source = Path::new(DEFAULT_SCRIPTS_SOURCE_DIR);
    let files = bundled_scripts(source, Path::new(DEFAULT_SCRIPTS_DEST_DIR));
    seed_files(
        Path::new(SEED_MANIFEST_PATH),
        &files,
        "delete it to take the new one",
    );
}

/// The tracked sample scripts (bot-players) scenarios attach, checked in under the
/// workspace's own `project/scripts/`.
pub const SAMPLE_SCRIPTS_DIR: &str = "project/scripts";

/// Seed a workspace the caller owns at `root` (#782): the bundled scripts, the UI
/// widget kit's scripts and the tracked samples, each written over whatever is
/// there, so it always holds the engine's current source. The headless harness
/// runs here, so a test never reads the developer's `./project`, where a stale or
/// edited copy is kept on purpose. Relative paths in `root` resolve as in `./`.
pub fn seed_workspace(root: &Path) {
    use crate::scene::authoring::ui_widgets::{parts::SCRIPT_DIR, SCRIPT_SOURCE_DIR};
    let dirs = [
        (DEFAULT_SCRIPTS_SOURCE_DIR, DEFAULT_SCRIPTS_DEST_DIR),
        (SCRIPT_SOURCE_DIR, SCRIPT_DIR),
        (SAMPLE_SCRIPTS_DIR, SAMPLE_SCRIPTS_DIR),
    ];
    for (source, dest) in dirs {
        let dest = root.join(dest);
        let files = bundled_scripts(Path::new(source), &dest);
        let written = std::fs::create_dir_all(&dest).and_then(|()| {
            files
                .iter()
                .try_for_each(|(to, code)| std::fs::write(to, code))
        });
        if let Err(e) = written {
            eprintln!("[Seed] seeding {} failed: {e}", dest.display());
        }
    }
}

/// The default scene document, byte for byte what Save would write for it.
fn default_scene_bytes() -> Result<Vec<u8>, String> {
    let mut scene = Scene::new();
    default_scene::build(&mut scene, default_scene::BOT_SCRIPT);
    scene_json(&scene).map(String::into_bytes)
}

/// Every `.lua` under `source`, paired with its destination under `dest`.
fn bundled_scripts(source: &Path, dest: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let Ok(entries) = std::fs::read_dir(source) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for path in entries.flatten().map(|e| e.path()) {
        let is_lua = path.extension().and_then(|e| e.to_str()) == Some("lua");
        let (Some(name), true) = (path.file_name(), is_lua) else {
            continue;
        };
        if let Ok(bytes) = std::fs::read(&path) {
            files.push((dest.join(name), bytes));
        }
    }
    files
}

/// Seed each `(destination, bundled bytes)` pair under one manifest load and save.
fn seed_files(manifest_path: &Path, files: &[(PathBuf, Vec<u8>)], hint: &str) {
    let mut manifest = SeedManifest::load(manifest_path);
    for (dest, bundled) in files {
        manifest.seed(dest, bundled, hint);
    }
    if let Err(e) = manifest.save() {
        eprintln!("[Seed] writing {} failed: {e}", manifest_path.display());
    }
}

#[cfg(test)]
#[path = "seed_tests.rs"]
mod tests;
