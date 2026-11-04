//! Reading a page list from disk into upload-ready levels (#1694), on whatever
//! thread asks: no GPU here, so the windowed renderer runs it on a worker.
//!
//! A `.ktx2` page is read as it is stored (BC6H irradiance, a BC7 direction page,
//! every mip) when the GPU has BC compression. Without it, or for a `.png` page (a
//! scene baked before #1694), the PNG is decoded on the CPU: RGBM to linear
//! `Rgba16Float` for irradiance, RGBA8 for directions, its mips made by the same
//! `lightmap::levels` the bake made the KTX2's with. The shader then reads linear
//! light either way, filtered linearly.

use std::path::Path;

use crate::asset::lightmap_page::{self, PageFormat};
use crate::scene::lighting::lightmap::{decode_direction, decode_rgbm, direction_levels, levels};

/// Which pages a list holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Kind {
    /// Irradiance (`LightmapSet::pages`).
    Colour,
    /// Directions (#810, `LightmapSet::directions`).
    Direction,
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
    Ok(match kind {
        Kind::Colour => {
            let light: Vec<_> = texels.map(decode_rgbm).collect();
            let chain = levels(&light, edge);
            let bytes = chain.iter().map(|l| lightmap_page::to_rgba16f(l)).collect();
            (wgpu::TextureFormat::Rgba16Float, edge, bytes)
        }
        Kind::Direction => {
            let dirs: Vec<_> = texels.map(decode_direction).collect();
            let chain = direction_levels(&dirs, edge);
            let bytes = chain.into_iter().map(|l| l.concat()).collect();
            (wgpu::TextureFormat::Rgba8Unorm, edge, bytes)
        }
    })
}
