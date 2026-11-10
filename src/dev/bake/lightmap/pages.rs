//! Writing a bake's pages (#438, #1694). The bake's lightmaps are packed
//! (`lightmap::pack`) and each page encoded ([`encode_pages`]: BC6H irradiance and
//! a BC7 direction page, every mip, plus the RGBM PNG fallback, compressed), which
//! is the slow part and needs no scene, so it runs on the bake's worker, counting
//! its pages into the bake's `BakeProgress` and stopping between two levels when
//! that is cancelled; then `write::apply_encoded` only writes the finished bytes
//! and points the scene at them, on the caller's thread.
//!
//! stored-format: the lightmap pages a bake encodes into `<scene>.lightmaps/`

use std::io::Cursor;

use crate::asset::lightmap_page::Page;
use crate::scene::lighting::lightmap::{
    check_page_count, colour_page, direction_page, encode_directions, encode_masks, encode_texels,
    pack, BakeProgress, Lightmap, LightmapEntry, MaskChannels, MAX_PAGES,
};

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

/// A `size`² RGBA8 page as a PNG file's bytes.
pub(super) fn png(size: u32, rgba: Vec<u8>) -> Result<Vec<u8>, String> {
    let image =
        image::RgbaImage::from_raw(size, size, rgba).ok_or("lightmap page size mismatch")?;
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| format!("encode a lightmap PNG: {e}"))?;
    Ok(bytes.into_inner())
}
