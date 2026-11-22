use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::looks::{checker_recipe, shader_recipe, FLOOR, SHADER_NAME};
use crate::authoring::material::{bake_maps, MAPS_DIR};
use rusty_scene::MaterialAsset;
use rusty_shadergen::rebake::{refresh, WRITES};
use rusty_shadergen::{bake_recipe, engine_shader_dir, DEFAULT_OUT_DIR};

pub fn seed_default_assets() {
    let (textures, shaders) = (PathBuf::from(MAPS_DIR), PathBuf::from(DEFAULT_OUT_DIR));
    if let Err(e) = seed_default_assets_into(&textures, &shaders) {
        eprintln!("[Scene] seeding the default scene's assets failed: {e}");
    }
}

pub fn seed_default_assets_into(textures: &Path, shaders: &Path) -> Result<(), String> {
    seed_checker(textures)?;
    seed_shader(shaders)
}

fn seed_checker(dir: &Path) -> Result<(), String> {
    let file = format!("{FLOOR}_base_color.png");
    // A copy on disk may be the user's edit: never overwritten.
    if dir.join(&file).exists() {
        return Ok(());
    }
    let staging = Staging::new(dir)?;
    let mut floor = MaterialAsset {
        maps_recipe: Some(checker_recipe()),
        ..MaterialAsset::default()
    };
    // The checker reads no image files, so the root it would read them from is moot.
    bake_maps(&mut floor, FLOOR, &rusty_core::project::root(), &staging.0)?;
    staging.publish(&[&file])
}

fn seed_shader(dir: &Path) -> Result<(), String> {
    let wgsl = dir.join(format!("{SHADER_NAME}.wgsl"));
    if wgsl.exists() {
        if WRITES {
            refresh(&wgsl);
        }
        return Ok(());
    }
    let out = dir.to_string_lossy();
    bake_recipe(&shader_recipe(), engine_shader_dir(), &out).map_err(|e| e.to_string())?;
    Ok(())
}

/// A folder beside the destination, so `rename` stays on one filesystem and is atomic; named per
/// process and call, as many threads and processes seed at once.
struct Staging(PathBuf, PathBuf);

impl Staging {
    fn new(dest: &Path) -> Result<Self, String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let name = format!(".seed-{}-{n}", std::process::id());
        let dir = dest.join(name);
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(Self(dir, dest.to_path_buf()))
    }

    fn publish(&self, files: &[&str]) -> Result<(), String> {
        for f in files {
            let (from, to) = (self.0.join(f), self.1.join(f));
            // lint: allow(atomic-write) publishes a file staged whole in this process's own folder
            std::fs::rename(&from, &to).map_err(|e| format!("{}: {e}", to.display()))?;
        }
        Ok(())
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}
