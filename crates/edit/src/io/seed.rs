use std::path::{Path, PathBuf};

use super::bundled::{self, STARTER_MATERIALS, STARTER_MATERIALS_PATH};
use super::manifest::{SeedManifest, SeedOutcome, SEED_MANIFEST_PATH};
use super::scene_json;
use crate::default_scene;
use rusty_scene::Scene;

pub const DEFAULT_SCENE_PATH: &str = "assets/scenes/default.scene";

pub const DEFAULT_SCRIPTS_DEST_DIR: &str = rusty_core::project::SCRIPTS_DIR;

/// File ▸ Reset Scene builds here, never from the seeded file, which may be an older
/// default or the user's edit.
pub fn build_default_scene(scene: &mut Scene) -> String {
    default_scene::seed_default_assets();
    scene.clear();
    default_scene::build(scene, default_scene::BOT_SCRIPT);
    DEFAULT_SCENE_PATH.to_string()
}

pub fn seed_default_scene() {
    let manifest = Path::new(SEED_MANIFEST_PATH);
    let dest = PathBuf::from(DEFAULT_SCENE_PATH);
    seed_default_scene_at(manifest, &dest, default_scene::seed_default_assets);
}

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

pub fn seed_default_scripts() {
    crate::authoring::ui::widgets::seed();
    let mut files = embedded(bundled::SCRIPTS, Path::new(DEFAULT_SCRIPTS_DEST_DIR));
    files.push((
        PathBuf::from(STARTER_MATERIALS_PATH),
        STARTER_MATERIALS.to_vec(),
    ));
    seed_files(
        Path::new(SEED_MANIFEST_PATH),
        &files,
        "delete it to take the new one",
    );
}

pub fn seed_project() -> rusty_asset::audio::Refresh {
    seed_default_scripts();
    seed_default_scene();
    rusty_asset::audio::refresh(Path::new(rusty_core::project::ASSETS_DIR))
}

/// To stderr: the `session` bins' stdout is their protocol.
pub fn print_refresh(report: &rusty_asset::audio::Refresh) {
    for (_, line) in report.lines() {
        eprintln!("{line}");
    }
}

/// Writes over whatever is there, without the manifest: the harness runs here and must
/// never read a stale or edited copy.
pub fn seed_workspace(root: &Path, samples: Option<&Path>) {
    use crate::authoring::ui::widgets::parts::SCRIPT_DIR;
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
        // lint: allow(asset-write) a scene, prefab or lighting document, read whole on load
        let written = std::fs::create_dir_all(parent).and_then(|()| std::fs::write(&to, bytes));
        if let Err(e) = written {
            eprintln!("[Seed] seeding {} failed: {e}", to.display());
        }
    }
}

pub(crate) fn embedded(files: &[(&str, &str)], dest: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    files
        .iter()
        .map(|(name, code)| (dest.join(name), code.as_bytes().to_vec()))
        .collect()
}

fn default_scene_bytes() -> Result<Vec<u8>, String> {
    let mut scene = Scene::new();
    default_scene::build(&mut scene, default_scene::BOT_SCRIPT);
    scene_json(&scene).map(String::into_bytes)
}

fn lua_files(source: &Path, dest: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let Ok(entries) = std::fs::read_dir(source) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for path in entries.flatten().map(|e| e.path()) {
        let is_lua = rusty_core::paths::has_extension(&path, "lua");
        let (Some(name), true) = (path.file_name(), is_lua) else {
            continue;
        };
        if let Ok(bytes) = std::fs::read(&path) {
            files.push((dest.join(name), bytes));
        }
    }
    files
}

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
