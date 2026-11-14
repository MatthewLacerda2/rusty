//! Reading a page list from disk into upload-ready levels (#1694), on whatever
//! thread asks: no GPU here, so the windowed renderer runs it on a worker.
//!
//! An irradiance `.ktx2` page is read as it is stored (BC6H, every mip) when the
//! GPU has BC compression. Without it, the RGBM PNG beside it is decoded on the
//! CPU to linear `Rgba16Float`, its mips made by the same `lightmap::levels` the
//! bake made the KTX2's with. A `.png` irradiance page that stands alone (a
//! progressive bake's preview, #1970, or a scene baked before #1694) uploads its
//! one level. The shader then reads linear light either way, filtered linearly.
//!
//! A direction page (#810) is an RGBA8 `.ktx2` alone, uploaded as it is stored
//! (`Rgba8Unorm`, the bake's own mips): no decode, no copy, no mip build at load,
//! with or without BC compression. It is never block-compressed: BC7 turned a
//! lamp's direction up to 5° off a texel, which a glossy surface's baked
//! highlight showed as a sawtooth (#1996). A progressive bake's preview direction
//! page (#1970) is an RGBA8 PNG alone and uploads its one level as it is, like
//! the colour preview beside it, so both read the same level at any `lod` and a
//! pass costs no decode or mip build. Every shadowmask page (#1695) is an RGBA8
//! PNG alone, its mips made here by the bake's own box filter
//! (`lightmap::mask_levels`).

use std::path::Path;

use rusty_asset::lightmap_page::{self, PageFormat};
use rusty_lighting::lightmap::{decode_mask, decode_rgbm, levels, mask_levels};

/// Which pages a list holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Kind {
    /// Irradiance (`LightmapSet::pages`).
    Colour,
    /// Directions (#810, `LightmapSet::directions`): an RGBA8 KTX2 as stored, or
    /// a preview's PNG alone, its one level as it is.
    Direction,
    /// Shadowmasks (#1695, `LightmapSet::masks`): an RGBA8 PNG alone, whose mips
    /// are made here.
    Mask,
}

/// A page list ready to upload: its format, edge, and per page every level's
/// bytes, largest first.
#[derive(Debug)]
pub(crate) struct Loaded {
    pub format: wgpu::TextureFormat,
    pub edge: u32,
    pub layers: Vec<Vec<Vec<u8>>>,
}

impl Loaded {
    /// The bytes its levels take, every page's: what the array will hold.
    pub fn bytes(&self) -> u64 {
        let level_bytes =
            |levels: &Vec<Vec<u8>>| levels.iter().map(|l| l.len() as u64).sum::<u64>();
        self.layers.iter().map(level_bytes).sum()
    }
}

/// Read every page of `pages`; `Err` naming the page when one is unreadable,
/// malformed, or does not match the others' size, format or level count.
pub(crate) fn load(pages: &[String], kind: Kind, bc: bool) -> Result<Loaded, String> {
    let mut loaded: Option<Loaded> = None;
    for path in pages {
        let (format, edge, levels) = page(path, kind, bc).map_err(|e| format!("{path}: {e}"))?;
        match &mut loaded {
            None => {
                loaded = Some(Loaded {
                    format,
                    edge,
                    layers: vec![levels],
                })
            }
            Some(l) => {
                let same = l.format == format && l.edge == edge;
                if !same || l.layers[0].len() != levels.len() {
                    return Err(format!(
                        "{path}: not the size and format of the other pages"
                    ));
                }
                l.layers.push(levels);
            }
        }
    }
    loaded.ok_or_else(|| "no pages".into())
}

/// One page's format, edge and levels.
fn page(
    path: &str,
    kind: Kind,
    bc: bool,
) -> Result<(wgpu::TextureFormat, u32, Vec<Vec<u8>>), String> {
    let is_ktx2 = Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ktx2"));
    let (stored, upload) = match kind {
        Kind::Direction if !is_ktx2 => return preview_direction_page(path),
        Kind::Mask => return mask_page(path),
        Kind::Direction => (PageFormat::Rgba8, wgpu::TextureFormat::Rgba8Unorm),
        Kind::Colour => (PageFormat::Bc6h, wgpu::TextureFormat::Bc6hRgbUfloat),
    };
    if is_ktx2 && (bc || stored == PageFormat::Rgba8) {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let page = lightmap_page::read(&bytes)?;
        if page.format != stored {
            return Err(format!(
                "a {:?} page where a {kind:?} page belongs",
                page.format
            ));
        }
        return Ok((upload, page.edge, page.levels));
    }
    // The PNG beside a KTX2 page, or the page itself.
    let png = match is_ktx2 {
        true => Path::new(path).with_extension("png"),
        false => Path::new(path).to_path_buf(),
    };
    let image = image::open(&png).map_err(|e| e.to_string())?.to_rgba8();
    let edge = image.width();
    if image.height() != edge || edge == 0 {
        return Err("not a square page".into());
    }
    let light: Vec<_> = image.pixels().map(|p| decode_rgbm(p.0)).collect();
    // A `.png` page alone keeps its one level (above).
    let chain = match is_ktx2 {
        true => levels(&light, edge),
        false => vec![light],
    };
    let bytes = chain.iter().map(|l| lightmap_page::to_rgba16f(l)).collect();
    Ok((wgpu::TextureFormat::Rgba16Float, edge, bytes))
}

/// A preview's direction PNG (#1970): its one level, uploaded as written.
fn preview_direction_page(path: &str) -> Result<(wgpu::TextureFormat, u32, Vec<Vec<u8>>), String> {
    let image = square_rgba8(path)?;
    Ok((
        wgpu::TextureFormat::Rgba8Unorm,
        image.width(),
        vec![image.into_raw()],
    ))
}

/// One shadowmask page (#1695): its PNG's occlusions with the mips the colour
/// pages carry (`lightmap::mask_levels`, the same box filter), as RGBA8.
fn mask_page(path: &str) -> Result<(wgpu::TextureFormat, u32, Vec<Vec<u8>>), String> {
    let image = square_rgba8(path)?;
    let edge = image.width();
    let masks: Vec<_> = image.pixels().map(|p| decode_mask(p.0)).collect();
    let bytes = mask_levels(&masks, edge)
        .into_iter()
        .map(|l| l.concat())
        .collect();
    Ok((wgpu::TextureFormat::Rgba8Unorm, edge, bytes))
}

/// The PNG at `path` as RGBA8; `Err` unless it is square and not empty.
fn square_rgba8(path: &str) -> Result<image::RgbaImage, String> {
    let image = image::open(path).map_err(|e| e.to_string())?.to_rgba8();
    if image.height() != image.width() || image.width() == 0 {
        return Err("not a square page".into());
    }
    Ok(image)
}
