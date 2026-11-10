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
//! Direction (#810) and shadowmask (#1695) pages are RGBA8 PNGs alone, their mips
//! made here by the bake's own box filter (`lightmap::direction_levels`,
//! `lightmap::mask_levels`), uploaded as `Rgba8Unorm`. A direction page is never
//! block-compressed: BC7 turned a lamp's direction up to 5° off a texel, which a
//! glossy surface's baked highlight showed as a sawtooth (#1996).

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
    /// Directions (#810, `LightmapSet::directions`): an RGBA8 PNG alone, whose
    /// mips are made here.
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
    match kind {
        Kind::Direction => return rgba8_page(path, decode_direction, direction_levels),
        Kind::Mask => return rgba8_page(path, decode_mask, mask_levels),
        Kind::Colour => {}
    }
    let is_ktx2 = Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ktx2"));
    if is_ktx2 && bc {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let page = lightmap_page::read(&bytes)?;
        if page.format != PageFormat::Bc6h {
            return Err(format!(
                "a {:?} page where a {kind:?} page belongs",
                page.format
            ));
        }
        return Ok((wgpu::TextureFormat::Bc6hRgbUfloat, page.edge, page.levels));
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

/// One RGBA8 page (a direction page, #810, or a shadowmask, #1695): its PNG's
/// texels, `decode`d, with the mips the colour pages carry (`levels`, the bake's
/// box filter), as RGBA8.
fn rgba8_page<T>(
    path: &str,
    decode: fn([u8; 4]) -> T,
    levels: fn(&[T], u32) -> Vec<Vec<[u8; 4]>>,
) -> Result<(wgpu::TextureFormat, u32, Vec<Vec<u8>>), String> {
    let image = image::open(path).map_err(|e| e.to_string())?.to_rgba8();
    let edge = image.width();
    if image.height() != edge || edge == 0 {
        return Err("not a square page".into());
    }
    let texels: Vec<T> = image.pixels().map(|p| decode(p.0)).collect();
    let bytes = levels(&texels, edge)
        .into_iter()
        .map(|l| l.concat())
        .collect();
    Ok((wgpu::TextureFormat::Rgba8Unorm, edge, bytes))
}
