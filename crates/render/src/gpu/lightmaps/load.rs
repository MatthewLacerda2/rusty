use std::path::Path;

use rusty_asset::lightmap_page::{self, PageFormat};
use rusty_scene::lighting::page::encode::{decode_mask, decode_rgbm};
use rusty_scene::lighting::page::{levels, mask_levels};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Kind {
    Colour,
    Direction,
    Mask,
}

#[derive(Debug)]
pub(crate) struct Loaded {
    pub format: wgpu::TextureFormat,
    pub edge: u32,
    pub layers: Vec<Vec<Vec<u8>>>,
}

impl Loaded {
    pub fn bytes(&self) -> u64 {
        let level_bytes =
            |levels: &Vec<Vec<u8>>| levels.iter().map(|l| l.len() as u64).sum::<u64>();
        self.layers.iter().map(level_bytes).sum()
    }
}

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
    let chain = match is_ktx2 {
        true => levels(&light, edge),
        false => vec![light],
    };
    let bytes = chain.iter().map(|l| lightmap_page::to_rgba16f(l)).collect();
    Ok((wgpu::TextureFormat::Rgba16Float, edge, bytes))
}

fn preview_direction_page(path: &str) -> Result<(wgpu::TextureFormat, u32, Vec<Vec<u8>>), String> {
    let image = square_rgba8(path)?;
    Ok((
        wgpu::TextureFormat::Rgba8Unorm,
        image.width(),
        vec![image.into_raw()],
    ))
}

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

fn square_rgba8(path: &str) -> Result<image::RgbaImage, String> {
    let image = image::open(path).map_err(|e| e.to_string())?.to_rgba8();
    if image.height() != image.width() || image.width() == 0 {
        return Err("not a square page".into());
    }
    Ok(image)
}
