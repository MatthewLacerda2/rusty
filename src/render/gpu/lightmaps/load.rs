//! Reading a page list from disk into upload-ready levels (#1694), on whatever
//! thread asks: no GPU here, so the windowed renderer runs it on a worker.
//!
//! A `.ktx2` page is read as it is stored (BC6H irradiance, a BC7 direction page,
//! every mip) when the GPU has BC compression. Without it, or for a `.png` page (a
//! scene baked before #1694), the PNG is decoded on the CPU: RGBM to linear
//! `Rgba16Float` for irradiance, RGBA8 for directions, its mips made by the same
//! `lightmap::levels` the bake made the KTX2's with. A `.png` page that stands
//! alone (a scene baked before #1694) uploads its one level: its layout (a
//! 1-texel ring, an unfilled background) cannot carry mips without bleeding. The
//! shader then reads linear light either way, filtered linearly.

use std::path::Path;

use crate::asset::lightmap_page::{self, PageFormat};
use crate::scene::lighting::lightmap::{
    decode_direction, decode_mask, decode_rgbm, direction_levels, levels, mask_levels,
};

/// Which pages a list holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Kind {
    /// Irradiance (`LightmapSet::pages`).
    Colour,
    /// Directions (#810, `LightmapSet::directions`).
    Direction,
    /// Shadowmasks (#1695, `LightmapSet::masks`): an RGBA8 PNG alone, whose mips
    /// are made here.
    Mask,
}

/// A page list ready to upload: its format, edge, and per page every level's
/// bytes, largest first.
pub(crate) struct Loaded {
    pub format: wgpu::TextureFormat,
    pub edge: u32,
    pub layers: Vec<Vec<Vec<u8>>>,
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
    if kind == Kind::Mask {
        return mask_page(path);
    }
    let is_ktx2 = Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ktx2"));
    if is_ktx2 && bc {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let page = lightmap_page::read(&bytes)?;
        let format = match (page.format, kind) {
            (PageFormat::Bc6h, Kind::Colour) => wgpu::TextureFormat::Bc6hRgbUfloat,
            (PageFormat::Bc7, Kind::Direction) => wgpu::TextureFormat::Bc7RgbaUnorm,
            (f, _) => return Err(format!("a {f:?} page where a {kind:?} page belongs")),
        };
        return Ok((format, page.edge, page.levels));
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
    let texels = image.pixels().map(|p| p.0);
    // A `.png` page is a scene baked before #1694: a 1-texel ring and no filled
    // background, a layout whose mips would bleed, so it keeps its one level.
    let mips = is_ktx2;
    Ok(match kind {
        Kind::Colour => {
            let light: Vec<_> = texels.map(decode_rgbm).collect();
            let chain = match mips {
                true => levels(&light, edge),
                false => vec![light],
            };
            let bytes = chain.iter().map(|l| lightmap_page::to_rgba16f(l)).collect();
            (wgpu::TextureFormat::Rgba16Float, edge, bytes)
        }
        Kind::Mask => unreachable!("a mask page is read by `mask_page`"),
        Kind::Direction => {
            let bytes = match mips {
                true => {
                    let dirs: Vec<_> = texels.map(decode_direction).collect();
                    let chain = direction_levels(&dirs, edge);
                    chain.into_iter().map(|l| l.concat()).collect()
                }
                false => vec![image.as_raw().clone()],
            };
            (wgpu::TextureFormat::Rgba8Unorm, edge, bytes)
        }
    })
}

/// One shadowmask page (#1695): its PNG's occlusions with the mips the colour
/// pages carry (`lightmap::mask_levels`, the same box filter), as RGBA8.
fn mask_page(path: &str) -> Result<(wgpu::TextureFormat, u32, Vec<Vec<u8>>), String> {
    let image = image::open(path).map_err(|e| e.to_string())?.to_rgba8();
    let edge = image.width();
    if image.height() != edge || edge == 0 {
        return Err("not a square page".into());
    }
    let masks: Vec<_> = image.pixels().map(|p| decode_mask(p.0)).collect();
    let chain = mask_levels(&masks, edge);
    let bytes = chain.into_iter().map(|l| l.concat()).collect();
    Ok((wgpu::TextureFormat::Rgba8Unorm, edge, bytes))
}
