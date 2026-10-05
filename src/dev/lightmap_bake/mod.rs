//! src/dev/lightmap_bake/ — the lightmap bake as an authoring action (#438).
//!
//! The one path behind `Lighting.BakeLightmaps()` and the editor's "Bake Lightmaps"
//! button, so the two never drift. It gathers the static scene, runs the CPU bake
//! (`scene::lighting::lightmap`, pure and seeded), writes each lightmap as an RGBM
//! PNG beside the scene, and points the scene's `lightmaps` at them. Saving the scene
//! then persists those references. Headless: no GPU is involved.
//!
//! The lightmaps are packed into atlas pages (`lightmap::atlas`), written to
//! `<scene file>.lightmaps/lightmap_<page>_<tag>.png`, one folder per scene, emptied
//! of old pages first so a rebake never leaves stale ones behind. `<tag>` hashes the
//! texels, so a rebake that changes a page changes its path and the renderer, which
//! caches pages by path, loads the new one.
//!
//! The bake splits in three so the editor can run the middle on a worker thread
//! ([`job`], #808): [`gather_bake_input`] borrows the scene, `bake` needs only
//! the owned [`BakeScene`], and [`apply_lightmaps`] writes the result back.
//!
//! Allowed deps: scene (the bake and its data), image (PNG I/O).

use std::path::{Path, PathBuf};

use glam::Vec3;

use crate::scene::lighting::lightmap::{
    bake, encode_texels, pack, BakeScene, BakeSettings, Lightmap, LightmapEntry, LightmapSet,
};
use crate::scene::Scene;

pub mod job;
pub use job::{BakeOutcome, LightmapBakeJob};

/// Suffix of the folder beside a scene file that holds its lightmaps.
pub const LIGHTMAP_DIR_SUFFIX: &str = ".lightmaps";

/// The lightmap folder for a scene: `foo.scene` → `foo.scene.lightmaps/`.
pub fn lightmap_dir(scene_path: &str) -> PathBuf {
    PathBuf::from(format!("{scene_path}{LIGHTMAP_DIR_SUFFIX}"))
}

/// Bake every static mesh with a lightmap UV and point the scene at the new pages.
/// Returns how many lightmaps were written (0 when nothing is lightmappable, which
/// also clears the scene's old references). Errors when the scene was never saved
/// (there is nowhere to write) or a file cannot be written.
pub fn bake_scene_lightmaps(
    scene: &mut Scene,
    scene_path: Option<&str>,
    settings: &BakeSettings,
) -> Result<usize, String> {
    let scene_path = scene_path.ok_or(UNSAVED)?;
    let maps = bake(&gather_bake_input(scene), settings);
    apply_lightmaps(scene, scene_path, &maps)
}

/// Why a bake cannot start: there is nowhere beside the scene to write.
pub const UNSAVED: &str = "save the scene before baking lightmaps";

/// The static scene flattened for the bake: owned data, free of the ECS.
pub fn gather_bake_input(scene: &mut Scene) -> BakeScene {
    scene.refresh_world_matrices();
    BakeScene::gather(scene, &texture_average)
}

/// Write `maps` beside `scene_path` and point the scene at them, replacing the
/// previous bake's pages. Returns how many lightmaps were written.
pub fn apply_lightmaps(
    scene: &mut Scene,
    scene_path: &str,
    maps: &[Lightmap],
) -> Result<usize, String> {
    let dir = lightmap_dir(scene_path);
    clear_old_lightmaps(&dir)?;
    scene.lightmaps.clear();
    if maps.is_empty() {
        return Ok(0);
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("lightmap folder: {e}"))?;
    let atlas = pack(maps);
    let mut set = LightmapSet::default();
    for (k, texels) in atlas.pages.iter().enumerate() {
        let bytes = encode_texels(texels);
        let path = dir.join(format!("lightmap_{k}_{:08x}.png", fnv1a(&bytes)));
        let size = atlas.page_size;
        let image =
            image::RgbaImage::from_raw(size, size, bytes).ok_or("lightmap page size mismatch")?;
        image
            .save(&path)
            .map_err(|e| format!("write {}: {e}", path.display()))?;
        // The scene document stores `/` paths on every OS, like its other assets.
        let page = path
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        set.pages.push(page);
    }
    set.entries = atlas
        .placements
        .iter()
        .map(|&(entity, page, scale_offset)| LightmapEntry {
            entity,
            page,
            scale_offset,
        })
        .collect();
    log::info!(
        "[LightmapBake] {} lightmaps in {} page(s) of {}² in {}",
        maps.len(),
        set.pages.len(),
        atlas.page_size,
        dir.display()
    );
    scene.lightmaps = set;
    Ok(maps.len())
}

/// FNV-1a over the texels, folded to 32 bits: the file name's content tag.
fn fnv1a(bytes: &[u8]) -> u32 {
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
        (h ^ b as u64).wrapping_mul(0x0100_0000_01b3)
    });
    (hash ^ (hash >> 32)) as u32
}

/// Remove the `lightmap_*.png` files a previous bake left in `dir`.
fn clear_old_lightmaps(dir: &Path) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(()); // never baked
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("lightmap_") && name.ends_with(".png") {
            std::fs::remove_file(entry.path()).map_err(|e| format!("remove {name}: {e}"))?;
        }
    }
    Ok(())
}

/// A texture's average linear colour (its texels are sRGB), for the bake's albedo
/// and emission. `None` when it cannot be read; the material colour then stands alone.
fn texture_average(path: &str) -> Option<Vec3> {
    let rgba = image::open(path).ok()?.to_rgba8();
    let count = (rgba.width() * rgba.height()).max(1) as f32;
    let to_linear = |b: u8| {
        let c = b as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let sum = rgba.pixels().fold(Vec3::ZERO, |acc, p| {
        acc + Vec3::new(to_linear(p[0]), to_linear(p[1]), to_linear(p[2]))
    });
    Some(sum / count)
}

#[cfg(test)]
mod job_tests;
#[cfg(test)]
mod tests;
