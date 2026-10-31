//! src/scene/default_scene/seed.rs — bake the default scene's texture and shader
//! into the open project (#667).
//!
//! The scene names `assets/textures/checker_base_color.png` and the
//! `default_rim` surface shader. The texture is written only when missing, so a
//! hand-edited copy is never overwritten (as for the seeded scripts). The shader is
//! baked when missing and re-baked from its recipe when stale (`shadergen::rebake`,
//! #1638); its recipe sits beside it as `default_rim.recipe.json`, so it can be
//! opened, edited and fed back to `Shader.Bake`.
//!
//! Every file is written whole and **renamed** into place (the texture through a
//! private staging directory, the shader by the bake itself). Tests and tools seed
//! from many threads and processes at once; a rename is atomic, so a reader sees the
//! whole file or none of it, never a half-written PNG.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::looks::{checker_recipe, shader_recipe, FLOOR, SHADER_NAME};
use crate::core::paths::native;
use crate::scene::authoring::material::{bake_maps, MAPS_DIR};
use crate::scene::MaterialAsset;
use crate::shadergen::rebake::{refresh, WRITES};
use crate::shadergen::{bake_recipe, engine_shader_dir, DEFAULT_OUT_DIR};

/// Seed the default texture and shader into `assets/`. A failure is logged
/// and boot carries on: the floor then draws the missing-texture checker and the
/// enemy the standard shader.
pub fn seed_default_assets() {
    let (textures, shaders) = (native(MAPS_DIR), native(DEFAULT_OUT_DIR));
    if let Err(e) = seed_default_assets_into(&textures, &shaders) {
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
    // The checker reads no image files, so the root it would read them from is moot.
    bake_maps(&mut floor, FLOOR, &crate::core::project::root(), &staging.0)?;
    staging.publish(&[&file])
}

/// Bake the default shader when missing; re-bake it when stale in a build that may
/// write (a failed re-bake is logged by `refresh`, and the scene draws the standard
/// shader until it composes; a shipped player re-composes it in memory at load).
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
