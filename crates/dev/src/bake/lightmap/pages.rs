//! stored-format: the lightmap pages a bake encodes into `<scene>.lightmaps/`

use std::io::Cursor;

use rusty_lighting::lightmap::{
    check_page_count, colour_page, direction_page, encode_masks, encode_texels, packing, AtlasPage,
    BakeProgress, Lightmap, MaskChannels, MAX_PAGES,
};
use rusty_scene::lighting::lightmap_set::LightmapEntry;

use super::spill::{Spill, Spilled};

#[derive(Debug, PartialEq)]
pub struct EncodedPages {
    pub maps: usize,
    pub page_size: u32,
    pub colour: Vec<(Spilled, Spilled)>,
    pub directions: Vec<Spilled>,
    pub entries: Vec<LightmapEntry>,
    pub lights: rusty_lighting::lightmap::TakenLights,
    pub masks: Vec<Spilled>,
    pub shadowmask: MaskChannels,
    pub gpu_bytes: usize,
    pub(super) spill: Option<Spill>,
}

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
        layout.on_page(p).for_each(|i| maps[i] = None);
        if put_page(page, (&mut encoded, &mut spill), progress, threads)?.is_none() {
            return Ok(None);
        }
        crate::memory::relieve();
    }
    encoded.spill = Some(spill);
    Ok(Some(encoded))
}

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

pub(super) fn png(size: u32, rgba: Vec<u8>) -> Result<Vec<u8>, String> {
    let image =
        image::RgbaImage::from_raw(size, size, rgba).ok_or("lightmap page size mismatch")?;
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| format!("encode a lightmap PNG: {e}"))?;
    Ok(bytes.into_inner())
}
