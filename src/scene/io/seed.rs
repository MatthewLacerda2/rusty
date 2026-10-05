//! src/scene/io/seed.rs — seed the engine's defaults into the open project.
//!
//! The default scene (built in Rust by `default_scene::build`, #667), the bundled
//! scripts and the starter materials (embedded, see `bundled`, #829) are seeded into
//! the project on boot, through the seed manifest (#746): a file nobody edited
//! follows the engine's current default, an edited one is kept. Every path here is
//! relative to the project root, the working directory (`core::project`).

use std::path::{Path, PathBuf};

use super::bundled::{self, STARTER_MATERIALS, STARTER_MATERIALS_PATH};
use super::manifest::{SeedManifest, SEED_MANIFEST_PATH};
use super::scene_json;
use crate::scene::{default_scene, Scene};

/// Where the default scene is seeded in the project.
pub const DEFAULT_SCENE_PATH: &str = "assets/scenes/default.scene";

/// Where the bundled scripts are seeded, so scenes referencing
/// `assets/scripts/<name>.lua` resolve on boot.
pub const DEFAULT_SCRIPTS_DEST_DIR: &str = "assets/scripts";

/// Build the engine's current default scene into a fresh `scene`, replacing what was
/// there, and return its path. File ▸ Reset Scene goes through here, never through
/// the seeded file, which may be an older default or the user's edit.
pub fn build_default_scene(scene: &mut Scene) -> String {
    default_scene::seed_default_assets();
    *scene = Scene::new();
    default_scene::build(scene, default_scene::BOT_SCRIPT);
    DEFAULT_SCENE_PATH.to_string()
}

/// Seed the default scene into the open project on boot, through the seed
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

/// Seed the bundled default scripts and the starter materials into the project on
/// boot, through the seed manifest. Idempotent. The UI widget kit's scripts and
/// prefabs are seeded too — those directories are engine-owned and rewritten (see
/// `authoring::ui_widgets::seed`).
pub fn seed_default_scripts() {
    crate::scene::authoring::ui_widgets::seed();
    let mut files = embedded(bundled::SCRIPTS, Path::new(DEFAULT_SCRIPTS_DEST_DIR));
    files.push((STARTER_MATERIALS_PATH.into(), STARTER_MATERIALS.to_vec()));
    seed_files(
        Path::new(SEED_MANIFEST_PATH),
        &files,
        "delete it to take the new one",
    );
}

/// Seed a workspace the caller owns at `root` (#782): the `.lua` files of `samples`
/// (a project's scripts folder, e.g. the test-fixture project's), then the bundled
/// scripts, the UI widget kit's scripts and the starter materials, each written over
/// whatever is there, so it always holds the engine's current source. The headless
/// harness runs here, so a test never reads a developer's project, where a stale or
/// edited copy is kept on purpose.
pub fn seed_workspace(root: &Path, samples: Option<&Path>) {
    use crate::scene::authoring::ui_widgets::parts::SCRIPT_DIR;
    let scripts = root.join(DEFAULT_SCRIPTS_DEST_DIR);
    let mut files = samples.map_or_else(Vec::new, |dir| lua_files(dir, &scripts));
    files.extend(embedded(bundled::SCRIPTS, &scripts));
    files.extend(embedded(bundled::UI_SCRIPTS, &root.join(SCRIPT_DIR)));
    files.push((
        root.join(STARTER_MATERIALS_PATH),
        STARTER_MATERIALS.to_vec(),
    ));
    for (to, bytes) in files {
        let parent = to.parent().unwrap_or(root);
        let written = std::fs::create_dir_all(parent).and_then(|()| std::fs::write(&to, bytes));
        if let Err(e) = written {
            eprintln!("[Seed] seeding {} failed: {e}", to.display());
        }
    }
}

/// Each embedded `(file name, source)` paired with its destination under `dest`.
pub(crate) fn embedded(files: &[(&str, &str)], dest: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    files
        .iter()
        .map(|(name, code)| (dest.join(name), code.as_bytes().to_vec()))
        .collect()
}

/// The default scene document, byte for byte what Save would write for it.
fn default_scene_bytes() -> Result<Vec<u8>, String> {
    let mut scene = Scene::new();
    default_scene::build(&mut scene, default_scene::BOT_SCRIPT);
    scene_json(&scene).map(String::into_bytes)
}

/// Every `.lua` under `source`, paired with its destination under `dest`.
fn lua_files(source: &Path, dest: &Path) -> Vec<(PathBuf, Vec<u8>)> {
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
