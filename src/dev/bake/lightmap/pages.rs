//! Writing a bake's pages (#438, #1694). The bake's lightmaps are packed
//! (`lightmap::pack`) and each page encoded ([`encode_pages`]: BC6H irradiance and
//! a BC7 direction page, every mip, plus the RGBM PNG fallback, compressed), which
//! is the slow part and needs no scene, so it runs on the bake's worker, counting
//! its pages into the bake's `BakeProgress` and stopping between two levels when
//! that is cancelled; then [`apply_encoded`] only writes the finished bytes and
//! points the scene at them, on the caller's thread.

use std::io::Cursor;
use std::path::Path;

use crate::asset::lightmap_page::{self, Page};
use crate::scene::lighting::lightmap::{
    check_page_count, colour_page, direction_page, encode_directions, encode_masks, encode_texels,
    pack, BakeProgress, Lightmap, LightmapEntry, LightmapSet, MaskChannels, MAX_PAGES,
};
use crate::scene::Scene;

use super::lightmap_dir;

/// A bake's pages encoded, ready to write.
#[derive(Debug, PartialEq)]
pub struct EncodedPages {
    /// How many lightmaps they hold.
    pub maps: usize,
    pub page_size: u32,
    /// Per colour page: its BC6H page and its RGBM PNG file's bytes.
    pub colour: Vec<(Page, Vec<u8>)>,
    /// Per direction page (#810): its BC7 page and its RGBA8 PNG file's bytes.
    pub directions: Vec<(Page, Vec<u8>)>,
    pub entries: Vec<LightmapEntry>,
    /// The lights the bake took in (`LightmapSet::lights`, #1750), set by whoever
    /// gathered its input (`taken_lights`); empty from [`encode_pages`].
    pub lights: std::collections::BTreeSet<u32>,
    /// Per shadowmask page (#1695): its RGBA8 PNG file's bytes. Empty unless the
    /// bake gave a light a channel.
    pub masks: Vec<Vec<u8>>,
    /// The bake's shadowmask channels (`BakeScene::shadowmask`), set like
    /// `lights`; empty from [`encode_pages`].
    pub shadowmask: MaskChannels,
}

/// Pack `maps` into pages of at most `max_page` texels a side (the bake's
/// `BakeSettings::page`, #1704) and encode them, every page on every core (the bands of a level are
/// split over them, so the blocks never depend on the machine), counting each
/// finished page into `progress`. `Ok(None)` when `progress` is cancelled first:
/// it is asked before every level of every page. `Err` before any page is drawn
/// when the bake would make more than [`MAX_PAGES`] (`check_page_count`): the
/// renderer could not load them, so the scene keeps its previous lightmaps.
pub fn encode_pages(
    maps: &[Lightmap],
    max_page: u32,
    progress: &BakeProgress,
) -> Result<Option<EncodedPages>, String> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    check_page_count(maps, max_page, MAX_PAGES)?;
    let atlas = pack(maps, max_page);
    let size = atlas.page_size;
    let total = atlas.pages.len() + atlas.directions.len() + atlas.masks.len();
    progress.start_pages(total as u64);
    let stop = || progress.is_cancelled();
    let mut colour = Vec::with_capacity(atlas.pages.len());
    for texels in &atlas.pages {
        let Some(page) = colour_page(texels, size, threads, &stop) else {
            return Ok(None);
        };
        colour.push((page, png(size, encode_texels(texels))?));
        progress.page_done();
    }
    let mut directions = Vec::with_capacity(atlas.directions.len());
    for dirs in &atlas.directions {
        let Some(page) = direction_page(dirs, size, threads, &stop) else {
            return Ok(None);
        };
        directions.push((page, png(size, encode_directions(dirs))?));
        progress.page_done();
    }
    // A mask page is its RGBA8 PNG alone: the loader makes its mips (#1695).
    let mut masks = Vec::with_capacity(atlas.masks.len());
    for mask in &atlas.masks {
        if stop() {
            return Ok(None);
        }
        masks.push(png(size, encode_masks(mask))?);
        progress.page_done();
    }
    let entries = atlas
        .placements
        .iter()
        .map(|&(entity, page, scale_offset)| LightmapEntry {
            entity,
            page,
            scale_offset,
        })
        .collect();
    Ok(Some(EncodedPages {
        maps: maps.len(),
        page_size: size,
        colour,
        directions,
        entries,
        lights: Default::default(),
        masks,
        shadowmask: MaskChannels::default(),
    }))
}

/// What one `size`² shadowmask page takes on the GPU: RGBA8 with its mips.
fn mask_gpu_bytes(size: u32) -> usize {
    let levels = crate::scene::lighting::lightmap::level_count(size);
    (0..levels).map(|l| ((size >> l) as usize).pow(2) * 4).sum()
}

/// A `size`² RGBA8 page as a PNG file's bytes.
fn png(size: u32, rgba: Vec<u8>) -> Result<Vec<u8>, String> {
    let image =
        image::RgbaImage::from_raw(size, size, rgba).ok_or("lightmap page size mismatch")?;
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| format!("encode a lightmap PNG: {e}"))?;
    Ok(bytes.into_inner())
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
    clear_old_lightmaps(&dir)?;
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
    std::fs::create_dir_all(&dir).map_err(|e| format!("lightmap folder: {e}"))?;
    let size = encoded.page_size;
    let mut set = LightmapSet::default();
    for (k, (page, png)) in encoded.colour.iter().enumerate() {
        set.pages.push(write_page(&dir, "lightmap", k, page, png)?);
    }
    for (k, (page, png)) in encoded.directions.iter().enumerate() {
        set.directions
            .push(write_page(&dir, "lightmap_dir", k, page, png)?);
    }
    for (k, png) in encoded.masks.iter().enumerate() {
        set.masks.push(write_png(&dir, "lightmap_mask", k, png)?);
    }
    set.extend(encoded.entries.iter().cloned());
    set.lights = encoded.lights.clone();
    set.channels = encoded.shadowmask.channels.clone();
    set.fell_back = encoded.shadowmask.fell_back.clone();
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

/// Remove the `lightmap_*` pages (`.ktx2` and `.png`) a previous bake left in `dir`.
fn clear_old_lightmaps(dir: &Path) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(()); // never baked
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let page = name.ends_with(".png") || name.ends_with(".ktx2");
        if name.starts_with("lightmap_") && page {
            std::fs::remove_file(entry.path()).map_err(|e| format!("remove {name}: {e}"))?;
            // Noted like a write, so the renderer frees the arrays holding it.
            crate::core::fs::written::note_written(&entry.path().to_string_lossy());
        }
    }
    Ok(())
}
