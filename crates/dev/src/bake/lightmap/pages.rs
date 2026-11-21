//! Writing a bake's pages (#438, #1694). The bake's lightmaps are laid out
//! (`lightmap::packing`) and each page composed, encoded and spilled to the
//! bake's scratch folder before the next is composed ([`encode_pages`], #2312:
//! BC6H irradiance, every mip, plus the RGBM PNG fallback, compressed; a direction
//! page as RGBA8, every mip, in a KTX2 alone; a shadowmask as an RGBA8 PNG alone,
//! whose mips the loader makes), so the page stage holds one page at a time, and a
//! lightmap is let go once its page is composed. That is the slow part and needs
//! no scene, so it runs on the bake's worker, counting its pages into the bake's
//! `BakeProgress` and stopping between two levels when that is cancelled; then
//! `write::apply_encoded` only links the spilled files into the scene's folder
//! and points the scene at them, on the caller's thread.
//!
//! stored-format: the lightmap pages a bake encodes into `<scene>.lightmaps/`

use std::io::Cursor;

use rusty_lighting::lightmap::{
    check_page_count, colour_page, direction_page, encode_masks, encode_texels, packing, AtlasPage,
    BakeProgress, Lightmap, MaskChannels, MAX_PAGES,
};
use rusty_scene::lighting::lightmap_set::LightmapEntry;

use super::spill::{Spill, Spilled};

/// A bake's pages encoded and spilled, ready to write.
#[derive(Debug, PartialEq)]
pub struct EncodedPages {
    /// How many lightmaps they hold.
    pub maps: usize,
    pub page_size: u32,
    /// Per colour page: its BC6H KTX2 file and its RGBM PNG fallback.
    pub colour: Vec<(Spilled, Spilled)>,
    /// Per direction page (#810): its RGBA8 levels, every mip, as a KTX2 file.
    /// Not block-compressed: BC7 turned a lamp's direction up to 5° off a texel,
    /// which a glossy surface's baked highlight showed as a sawtooth (#1996).
    pub directions: Vec<Spilled>,
    pub entries: Vec<LightmapEntry>,
    /// The lights the bake took in, each with the mode it baked as
    /// (`LightmapSet::lights` and `baked_as`, #1750, #2522), set by whoever
    /// gathered its input (`taken_lights`); empty from [`encode_pages`].
    pub lights: rusty_lighting::lightmap::TakenLights,
    /// Per shadowmask page (#1695): its RGBA8 PNG file. Empty unless the bake
    /// gave a light a channel.
    pub masks: Vec<Spilled>,
    /// The bake's shadowmask channels (`BakeScene::shadowmask`), set like
    /// `lights`; empty from [`encode_pages`].
    pub shadowmask: MaskChannels,
    /// What the pages take on the GPU, every mip: the colour and direction pages
    /// as stored, each shadowmask page as RGBA8 with the mips made on load.
    pub gpu_bytes: usize,
    /// The folder the files lie in until written; removed when these are dropped.
    pub(super) spill: Option<Spill>,
}

/// Lay `maps` out on pages of at most `max_page` texels a side (the bake's
/// `BakeSettings::page`, #1704) and encode them, spilled into the scratch
/// folder of `scene_path`'s project (`spill`), every page on every core (the bands of a level are split over
/// them, so the blocks never depend on the machine), counting each finished page
/// into `progress`. One page is composed at a time, and each map is dropped once
/// its page is. `Ok(None)` when `progress` is cancelled first: it is asked before
/// every level of every page. `Err` before any page is drawn when the bake would
/// make more than [`MAX_PAGES`] (`check_page_count`): the renderer could not load
/// them, so the scene keeps its previous lightmaps.
pub fn encode_pages(
    maps: Vec<Lightmap>,
    max_page: u32,
    progress: &BakeProgress,
    scene_path: &str,
) -> Result<Option<EncodedPages>, String> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    check_page_count(&maps, max_page, MAX_PAGES)?;
    let layout = packing(&maps, max_page);
    let (size, count) = (layout.page_size, layout.page_count);
    let kinds = 1 + usize::from(layout.directional) + usize::from(layout.masked);
    progress.start_pages((count * kinds) as u64);
    let entries = layout
        .placements(&maps)
        .into_iter()
        .map(|(entity, page, scale_offset)| LightmapEntry {
            entity,
            page,
            scale_offset,
        })
        .collect();
    let mut encoded = EncodedPages {
        maps: maps.len(),
        page_size: size,
        colour: Vec::with_capacity(count),
        directions: Vec::new(),
        entries,
        lights: Default::default(),
        masks: Vec::new(),
        shadowmask: MaskChannels::default(),
        gpu_bytes: 0,
        spill: None,
    };
    if count == 0 {
        return Ok(Some(encoded));
    }
    let mut spill = Spill::open(scene_path)?;
    let mut maps: Vec<Option<Lightmap>> = maps.into_iter().map(Some).collect();
    for p in 0..count {
        let page = layout.page(p, &|i| maps[i].as_ref().expect("on one page"));
        // Each map lies on one page: once it is composed, the map is let go.
        layout.on_page(p).for_each(|i| maps[i] = None);
        if put_page(page, (&mut encoded, &mut spill), progress, threads)?.is_none() {
            return Ok(None);
        }
        // The page and its maps are freed: hand them back (`memory::relieve`), so
        // the next page's do not stack on them, and the last page's end the stage.
        crate::memory::relieve();
    }
    encoded.spill = Some(spill);
    Ok(Some(encoded))
}

/// Encode `page` (of `encoded.page_size`) on `threads`, spill its files into
/// `spill` and list them in `encoded`, counting each kind done into `progress`;
/// `None` when that is cancelled first.
fn put_page(
    page: AtlasPage,
    (encoded, spill): (&mut EncodedPages, &mut Spill),
    progress: &BakeProgress,
    threads: usize,
) -> Result<Option<()>, String> {
    let size = encoded.page_size;
    let stop = || progress.is_cancelled();
    if stop() {
        return Ok(None);
    }
    let Some(colour) = colour_page(&page.texels, size, threads, &stop) else {
        return Ok(None);
    };
    let fallback = png(size, encode_texels(&page.texels))?;
    drop(page.texels);
    encoded.gpu_bytes += colour.gpu_bytes();
    let ktx2 = rusty_asset::lightmap_page::write(&colour);
    encoded
        .colour
        .push((spill.put(&ktx2)?, spill.put(&fallback)?));
    progress.page_done();
    if let Some(dirs) = page.directions {
        if stop() {
            return Ok(None);
        }
        let dirs = direction_page(&dirs, size);
        encoded.gpu_bytes += dirs.gpu_bytes();
        let ktx2 = rusty_asset::lightmap_page::write(&dirs);
        encoded.directions.push(spill.put(&ktx2)?);
        progress.page_done();
    }
    // A mask page is its RGBA8 PNG alone: the loader makes its mips.
    if let Some(mask) = page.masks {
        if stop() {
            return Ok(None);
        }
        encoded
            .masks
            .push(spill.put(&png(size, encode_masks(&mask))?)?);
        let levels = rusty_lighting::lightmap::level_count(size);
        encoded.gpu_bytes += (0..levels)
            .map(|l| ((size >> l) as usize).pow(2) * 4)
            .sum::<usize>();
        progress.page_done();
    }
    Ok(Some(()))
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
