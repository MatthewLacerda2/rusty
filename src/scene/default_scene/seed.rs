//! src/scene/default_scene/seed.rs — bake the default scene's texture and shader
//! into the project workspace (#667).
//!
//! The scene names `project/assets/textures/checker_base_color.png` and the
//! `default_rim` surface shader; this writes them, each only when missing, so a
//! hand-edited or re-baked copy is never overwritten (as for the seeded scripts).
//! The shader's recipe is written beside it as `default_rim.recipe.json`, so it can be
//! opened, edited and fed back to `Shader.Bake`.
//!
//! Every file is baked into a private staging directory and **renamed** into place.
//! Tests and tools seed from many threads and processes at once; a rename is atomic,
//! so a reader sees the whole file or none of it, never a half-written PNG.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::looks::{checker_recipe, shader_recipe, FLOOR, SHADER_NAME};
use crate::scene::authoring::material::{bake_maps, MAPS_DIR};
use crate::scene::MaterialAsset;
use crate::shadergen::{bake_recipe, DEFAULT_OUT_DIR, ENGINE_SHADER_DIR};

/// Seed the default texture and shader into `project/assets/`. A failure is logged
/// and boot carries on: the floor then draws the missing-texture checker and the
/// enemy the standard shader.
pub fn seed_default_assets() {
    let (textures, shaders) = (Path::new(MAPS_DIR), Path::new(DEFAULT_OUT_DIR));
    if let Err(e) = seed_default_assets_into(textures, shaders) {
        eprintln!("[Scene] seeding the default scene's assets failed: {e}");
    }
}

/// Seed into explicit directories (tests point these at a temp dir).
pub fn seed_default_assets_into(textures: &Path, shaders: &Path) -> Result<(), String> {
    seed_checker(textures)?;
    seed_shader(shaders)
}

fn seed_checker(dir: &Path) -> Result<(), String> {
    let file = format!("{FLOOR}_base_color.png");
    if dir.join(&file).exists() {
        return Ok(());
    }
    let staging = Staging::new(dir)?;
    let mut floor = MaterialAsset {
        maps_recipe: Some(checker_recipe()),
        ..MaterialAsset::default()
    };
    bake_maps(&mut floor, FLOOR, &staging.0)?;
    staging.publish(&[&file])
}

fn seed_shader(dir: &Path) -> Result<(), String> {
    let wgsl = format!("{SHADER_NAME}.wgsl");
    if dir.join(&wgsl).exists() {
        return Ok(());
    }
    let staging = Staging::new(dir)?;
    let recipe = shader_recipe();
    let out = staging.0.to_string_lossy();
    bake_recipe(&recipe, ENGINE_SHADER_DIR, &out).map_err(|e| e.to_string())?;
    let json_name = format!("{SHADER_NAME}.recipe.json");
    std::fs::write(staging.0.join(&json_name), recipe.to_json()?).map_err(|e| e.to_string())?;
    // The module last: the renderer keys on `<name>.wgsl`, so its params are there first.
    let params = format!("{SHADER_NAME}.params.json");
    staging.publish(&[&json_name, &params, &wgsl])
}

/// A unique directory beside the destination (same filesystem, so `rename` is
/// atomic), removed when dropped.
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

    /// Move each staged file into the destination, in order.
    fn publish(&self, files: &[&str]) -> Result<(), String> {
        for f in files {
            let (from, to) = (self.0.join(f), self.1.join(f));
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
