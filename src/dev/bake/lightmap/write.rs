//! Writing a bake's encoded pages beside its scene (#438, #1694) and pointing the
//! scene at them ([`apply_encoded`]), and a progressive bake's preview pages
//! (#1970, `lightmap_preview_*` in the same folder, [`write_set`]), which the
//! finished bake's write clears with the rest.
//!
//! stored-format: the lightmap pages a bake writes into `<scene>.lightmaps/`

use std::path::Path;

use crate::asset::lightmap_page::{self, Page};
use crate::scene::lighting::lightmap::{BakeProgress, Lightmap, LightmapSet};
use crate::scene::Scene;

use super::{encode_pages, lightmap_dir, EncodedPages};

/// The file names a finished bake's pages start with.
pub(super) const FINAL: &str = "lightmap";
/// The file names a progressive bake's preview pages start with (#1970).
pub(super) const PREVIEW: &str = "lightmap_preview";

/// What one `size`² shadowmask page takes on the GPU: RGBA8 with its mips.
fn mask_gpu_bytes(size: u32) -> usize {
    let levels = crate::scene::lighting::lightmap::level_count(size);
    (0..levels).map(|l| ((size >> l) as usize).pow(2) * 4).sum()
}

/// [`encode_pages`] then [`apply_encoded`]: write `maps` beside `scene_path`, on
/// pages of the scene's Max Lightmap Size.
pub fn apply_lightmaps(
    scene: &mut Scene,
    scene_path: &str,
    maps: &[Lightmap],
) -> Result<usize, String> {
    let page = scene.lighting_settings.lightmaps.page();
    let encoded = encode_pages(maps, page, &BakeProgress::default())?;
    apply_encoded(scene, scene_path, &encoded.ok_or("never cancelled")?)
}

/// Write `encoded` beside `scene_path` and point the scene at it, replacing the
/// previous bake's pages. Returns how many lightmaps were written.
pub fn apply_encoded(
    scene: &mut Scene,
    scene_path: &str,
    encoded: &EncodedPages,
) -> Result<usize, String> {
    let dir = lightmap_dir(scene_path);
    clear_pages(&dir, FINAL)?;
    scene.lightmaps.clear();
    if encoded.maps == 0 {
        // No lightmap, but the probes still mask by these channels (#1695).
        let mut set = LightmapSet::default();
        set.channels = encoded.shadowmask.channels.clone();
        set.fell_back = encoded.shadowmask.fell_back.clone();
        warn_fell_back(&set);
        scene.lightmaps = set.into();
        return Ok(0);
    }
    let size = encoded.page_size;
    let set = write_set(&dir, FINAL, encoded)?;
    warn_fell_back(&set);
    let mask_bytes = encoded.masks.len() * mask_gpu_bytes(size);
    let bytes: usize = (encoded.colour.iter())
        .chain(&encoded.directions)
        .map(|(p, _)| p.gpu_bytes())
        .sum::<usize>()
        + mask_bytes;
    log::info!(
        "[LightmapBake] {} lightmaps in {} page(s) of {size}²{}, {:.1} MiB on the GPU, in {}",
        encoded.maps,
        set.pages.len(),
        match (set.directions.is_empty(), set.masks.is_empty()) {
            (true, true) => "",
            (false, true) => " (directional)",
            (true, false) => " (shadowmask)",
            (false, false) => " (directional, shadowmask)",
        },
        bytes as f64 / (1024.0 * 1024.0),
        dir.display()
    );
    scene.lightmaps = set.into();
    Ok(encoded.maps)
}

/// Write `encoded`'s pages into `dir` under names starting `prefix` ([`FINAL`]
/// or [`PREVIEW`]), and the set that points at them.
pub(super) fn write_set(
    dir: &Path,
    prefix: &str,
    encoded: &EncodedPages,
) -> Result<LightmapSet, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("lightmap folder: {e}"))?;
    let mut set = LightmapSet::default();
    for (k, (page, png)) in encoded.colour.iter().enumerate() {
        set.pages.push(write_page(dir, prefix, k, page, png)?);
    }
    for (k, (page, png)) in encoded.directions.iter().enumerate() {
        let name = format!("{prefix}_dir");
        set.directions.push(write_page(dir, &name, k, page, png)?);
    }
    for (k, png) in encoded.masks.iter().enumerate() {
        set.masks
            .push(write_png(dir, &format!("{prefix}_mask"), k, png)?);
    }
    set.extend(encoded.entries.iter().cloned());
    set.lights = encoded.lights.clone();
    set.channels = encoded.shadowmask.channels.clone();
    set.fell_back = encoded.shadowmask.fell_back.clone();
    Ok(set)
}

/// Write one page as `<dir>/<prefix>_<k>_<tag>.ktx2` and its PNG fallback beside
/// it, and return the KTX2 path the scene stores.
fn write_page(
    dir: &Path,
    prefix: &str,
    k: usize,
    page: &Page,
    png: &[u8],
) -> Result<String, String> {
    let ktx2 = lightmap_page::write(page);
    let stem = dir.join(format!("{prefix}_{k}_{:08x}", content_tag(&ktx2)));
    let (ktx2_path, png_path) = (stem.with_extension("ktx2"), stem.with_extension("png"));
    std::fs::write(&png_path, png).map_err(|e| format!("write {}: {e}", png_path.display()))?;
    std::fs::write(&ktx2_path, &ktx2).map_err(|e| format!("write {}: {e}", ktx2_path.display()))?;
    for path in [&png_path, &ktx2_path] {
        crate::core::fs::written::note_written(&path.to_string_lossy());
    }
    // The scene document stores `/` paths on every OS, like its other assets (#819).
    Ok(crate::core::paths::to_slash(&ktx2_path))
}

/// Write one shadowmask page as `<dir>/<prefix>_<k>_<tag>.png` and return its path.
fn write_png(dir: &Path, prefix: &str, k: usize, png: &[u8]) -> Result<String, String> {
    let path = dir.join(format!("{prefix}_{k}_{:08x}.png", content_tag(png)));
    std::fs::write(&path, png).map_err(|e| format!("write {}: {e}", path.display()))?;
    crate::core::fs::written::note_written(&path.to_string_lossy());
    Ok(crate::core::paths::to_slash(&path))
}

/// Say, once per bake, which `Mixed` lights found no shadowmask channel and were
/// baked as `Baked` (#1695): more than four overlapped there.
fn warn_fell_back(set: &LightmapSet) {
    if set.fell_back.is_empty() {
        return;
    }
    log::warn!(
        "[LightmapBake] {} Mixed light(s) overlap 4 others and were baked as Baked \
         (no shadowmask channel left): entities {:?}. Turn some to Baked, or shrink \
         their range.",
        set.fell_back.len(),
        set.fell_back
    );
}

/// The page's content hash, folded to 32 bits: the file name's content tag.
pub(super) fn content_tag(bytes: &[u8]) -> u32 {
    let hash = crate::core::hash::content(bytes);
    (hash ^ (hash >> 32)) as u32
}

/// Remove the pages (`.ktx2` and `.png`) whose names start `<prefix>_` from `dir`:
/// under [`FINAL`] every page a previous bake left, its previews included.
pub(super) fn clear_pages(dir: &Path, prefix: &str) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(()); // never baked
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let page = name.ends_with(".png") || name.ends_with(".ktx2");
        if name.starts_with(&format!("{prefix}_")) && page {
            std::fs::remove_file(entry.path()).map_err(|e| format!("remove {name}: {e}"))?;
            // Noted like a write, so the renderer frees the arrays holding it.
            crate::core::fs::written::note_written(&entry.path().to_string_lossy());
        }
    }
    Ok(())
}
