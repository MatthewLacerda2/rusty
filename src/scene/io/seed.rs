//! src/scene/io/seed.rs — seed the engine's defaults into the open project.
//!
//! The default scene (built in Rust by `default_scene::build`, #667), the bundled
//! scripts and the starter materials (embedded, see `bundled`, #829) are seeded into
//! the project on boot, through the seed manifest (#746): a file nobody edited
//! follows the engine's current default, an edited one is kept, and a default
//! scene the user deleted stays deleted (#1618). Every path here is
//! relative to the project root, the working directory (`core::project`).

use std::path::{Path, PathBuf};

use super::bundled::{self, STARTER_MATERIALS, STARTER_MATERIALS_PATH};
use super::manifest::{SeedManifest, SeedOutcome, SEED_MANIFEST_PATH};
use super::scene_json;
use crate::core::paths::native;
use crate::scene::{default_scene, Scene};

/// Where the default scene is seeded in the project.
pub const DEFAULT_SCENE_PATH: &str = "assets/scenes/default.scene";

/// Where the bundled scripts are seeded, so scenes referencing
/// `assets/scripts/<name>.lua` resolve on boot.
pub const DEFAULT_SCRIPTS_DEST_DIR: &str = crate::core::project::SCRIPTS_DIR;

/// Build the engine's current default scene into a fresh `scene`, replacing what was
/// there, and return its path. File ▸ Reset Scene goes through here, never through
/// the seeded file, which may be an older default or the user's edit.
pub fn build_default_scene(scene: &mut Scene) -> String {
    default_scene::seed_default_assets();
    scene.clear();
    default_scene::build(scene, default_scene::BOT_SCRIPT);
    DEFAULT_SCENE_PATH.to_string()
}

/// Seed the default scene into the open project on boot, through the seed
/// manifest's [`seed_once`](SeedManifest::seed_once): a new project gets it, an
/// unedited copy follows the engine's default, and a deleted one stays deleted
/// (#1618), as Unity never recreates a template's `SampleScene`. Its texture and
/// shader are seeded with it, each only if missing (the shader also re-baked from
/// its recipe when stale, #1638), and not at all once the scene
/// is deleted (File ▸ Reset Scene still brings both back). Which scene then opens
/// is `core::project::boot_scene`'s call, not this one's.
pub fn seed_default_scene() {
    let manifest = Path::new(SEED_MANIFEST_PATH);
    let dest = native(DEFAULT_SCENE_PATH);
    seed_default_scene_at(manifest, &dest, default_scene::seed_default_assets);
}

/// [`seed_default_scene`] against explicit paths; `seed_assets` seeds the scene's
/// texture and shader, run unless the scene's outcome is
/// [`SeedOutcome::Deleted`].
fn seed_default_scene_at(
    manifest_path: &Path,
    dest: &Path,
    seed_assets: impl FnOnce(),
) -> Option<SeedOutcome> {
    let json = match default_scene_bytes() {
        Ok(json) => json,
        Err(e) => {
            eprintln!("[Scene] seeding the default scene failed: {e}");
            return None;
        }
    };
    let mut manifest = SeedManifest::load(manifest_path);
    let hint = "File ▸ Reset Scene to take the new one";
    let outcome = manifest.seed_once(dest, &json, hint);
    if let Err(e) = manifest.save() {
        eprintln!("[Seed] writing {} failed: {e}", manifest_path.display());
    }
    if outcome != SeedOutcome::Deleted {
        seed_assets();
    }
    Some(outcome)
}

/// Seed the bundled default scripts and the starter materials into the project on
/// boot, through the seed manifest. Idempotent. The UI widget kit's scripts and
/// prefabs are seeded too — those directories are engine-owned and rewritten (see
/// `authoring::ui::widgets::seed`).
pub fn seed_default_scripts() {
    crate::scene::authoring::ui::widgets::seed();
    let mut files = embedded(bundled::SCRIPTS, &native(DEFAULT_SCRIPTS_DEST_DIR));
    files.push((native(STARTER_MATERIALS_PATH), STARTER_MATERIALS.to_vec()));
    seed_files(
        Path::new(SEED_MANIFEST_PATH),
        &files,
        "delete it to take the new one",
    );
}

/// Bring the open project's engine-seeded files and its audio up to date, as one
/// step every authoring entry point runs on the project it opens: the editor's
/// open, the export (`dev::export`), the headless play-test
/// (`Harness::in_user_workspace`) and the `session` bins. That is the bundled
/// scripts, the starter materials and the UI widget kit
/// ([`seed_default_scripts`]), `default.scene` where the seed manifest says it is
/// due ([`seed_default_scene`]), and a `.wav` for each `.mp3` under `assets/`
/// (`asset::audio::refresh`). The standalone player runs none of it: it boots
/// read-only (#1653). The refresh's report is returned for the caller to print
/// (`Refresh::lines`) or, the export, to refuse on.
pub fn seed_project() -> crate::asset::audio::Refresh {
    seed_default_scripts();
    seed_default_scene();
    crate::asset::audio::refresh(&native(crate::core::project::ASSETS_DIR))
}

/// Print a [`seed_project`] refresh, one line per MP3, to stderr: the `session`
/// bins' stdout is their protocol, so no caller prints there. Every caller prints
/// through here, so the lines read the same whichever entry point opened the
/// project.
pub fn print_refresh(report: &crate::asset::audio::Refresh) {
    for (_, line) in report.lines() {
        eprintln!("{line}");
    }
}

/// Seed a workspace the caller owns at `root` (#782): the `.lua` files of `samples`
/// (a project's scripts folder, e.g. the test-fixture project's), then the bundled
/// scripts, the UI widget kit's scripts and the starter materials, each written over
/// whatever is there, so it always holds the engine's current source. The headless
/// harness runs here, so a test never reads a developer's project, where a stale or
/// edited copy is kept on purpose.
pub fn seed_workspace(root: &Path, samples: Option<&Path>) {
    use crate::scene::authoring::ui::widgets::parts::SCRIPT_DIR;
    let scripts = root.join(native(DEFAULT_SCRIPTS_DEST_DIR));
    let mut files = samples.map_or_else(Vec::new, |dir| lua_files(dir, &scripts));
    files.extend(embedded(bundled::SCRIPTS, &scripts));
    files.extend(embedded(
        bundled::UI_SCRIPTS,
        &root.join(native(SCRIPT_DIR)),
    ));
    files.push((
        root.join(native(STARTER_MATERIALS_PATH)),
        STARTER_MATERIALS.to_vec(),
    ));
    for (to, bytes) in files {
        let parent = to.parent().unwrap_or(root);
        // lint: allow(asset-write) a scene, prefab or lighting document, read whole on load
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
        let is_lua = crate::core::paths::has_extension(&path, "lua");
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
