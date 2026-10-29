//! A contact sheet of every node of a recipe (#1486), the PNG `Texture.Preview`
//! writes.
//!
//! The agent's stand-in for the live thumbnails Blender draws on each node: one
//! image with every node's output shrunk to a tile whose longer side is `size`, in
//! `recipe.nodes` order, left to right then top to bottom, in a near-square grid. A
//! non-square recipe's tiles keep its aspect (#1493). No text is drawn; the
//! caller maps tile `k` to the `k`th node. Each tile is taken through
//! [`super::evaluate_each`] as its node finishes, so only the small tiles are held.
//!
//! Every tile is sRGB-encoded, data included, the way Blender's previews pass
//! everything through the view transform: a 0.5 mask shows as 188, not 128. Alpha
//! is not shown; the sheet is opaque.

use image::{ImageBuffer, Rgba};

use super::evaluate_each;
use crate::core::color::linear_to_srgb;
use crate::procgen::image_buf::{Image, Size};
use crate::procgen::recipe::TextureRecipe;
use crate::procgen::source::Sources;

/// A rendered contact sheet: the encoded pixels and the grid's column count.
pub struct Sheet {
    pub pixels: ImageBuffer<Rgba<u8>, Vec<u8>>,
    pub columns: u32,
}

/// Render every node of `recipe`, its `image` nodes reading `sources`, to a contact
/// sheet of tiles whose longer side is `size`. A `size` above the recipe's longer
/// side is refused: a tile larger than the image adds nothing, and an unbounded one
/// could not be allocated.
pub fn sheet(recipe: &TextureRecipe, sources: &Sources, size: u32) -> Result<Sheet, String> {
    let size = size.max(1);
    let extent = recipe.extent();
    if size > extent.longer() {
        return Err(format!(
            "size {size} is larger than the recipe's longer side {}",
            extent.longer()
        ));
    }
    let tile = tile_size(extent, size);
    let n = recipe.nodes.len() as u32;
    let columns = (1..=n).find(|c| c * c >= n).unwrap_or(1);
    let rows = n.div_ceil(columns);
    let too_big = || {
        let (w, h) = (tile.width, tile.height);
        format!("a {columns}×{rows} sheet of {w}×{h} tiles is too large")
    };
    let width = columns.checked_mul(tile.width).ok_or_else(too_big)?;
    let height = rows.checked_mul(tile.height).ok_or_else(too_big)?;
    let mut tiles: Vec<Option<Vec<[u8; 3]>>> = recipe.nodes.iter().map(|_| None).collect();
    evaluate_each(recipe, sources, |i, img| {
        tiles[i] = Some(thumbnail(img, tile))
    })
    .map_err(|e| e.to_string())?;
    let mut pixels = ImageBuffer::from_pixel(width, height, Rgba([0, 0, 0, 255]));
    for (k, pixels_of) in tiles.into_iter().enumerate() {
        let pixels_of = pixels_of.expect("evaluate_each visits every node");
        let (col, row) = (k as u32 % columns, k as u32 / columns);
        let (ox, oy) = (col * tile.width, row * tile.height);
        for (j, [r, g, b]) in pixels_of.into_iter().enumerate() {
            let (x, y) = (j as u32 % tile.width, j as u32 / tile.width);
            pixels.put_pixel(ox + x, oy + y, Rgba([r, g, b, 255]));
        }
    }
    Ok(Sheet { pixels, columns })
}

/// Render `recipe`'s contact sheet and write it as a PNG at `path`, noting the
/// write like a bake does (#689). Returns the grid's column count.
pub fn write_preview(
    recipe: &TextureRecipe,
    sources: &Sources,
    path: &str,
    size: u32,
) -> Result<u32, String> {
    let sheet = sheet(recipe, sources, size)?;
    sheet.pixels.save(path).map_err(|e| e.to_string())?;
    crate::core::fs::written::note_written(path);
    Ok(sheet.columns)
}

/// A tile of `extent`'s aspect with its longer side at `size`, each side at least
/// one pixel.
pub fn tile_size(extent: Size, size: u32) -> Size {
    let longer = u64::from(extent.longer().max(1));
    let side = |s: u32| ((u64::from(s) * u64::from(size) / longer) as u32).max(1);
    Size::new(side(extent.width), side(extent.height))
}

/// `img` resampled to `tile`, sRGB-encoded: each tile pixel is the linear mean of
/// the source pixels it covers (a box filter), or the nearest one when enlarging.
pub fn thumbnail(img: &Image, tile: Size) -> Vec<[u8; 3]> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let (tw, th) = (tile.width as usize, tile.height as usize);
    let span = |t: usize, from: usize, to: usize| {
        let start = t * from / to;
        start..((t + 1) * from / to).max(start + 1)
    };
    let mut out = Vec::with_capacity(tile.pixels());
    for ty in 0..th {
        for tx in 0..tw {
            let mut sum = [0.0f32; 3];
            let (ys, xs) = (span(ty, h, th), span(tx, w, tw));
            let count = (ys.len() * xs.len()) as f32;
            for y in ys {
                for p in &img.pixels()[y * w + xs.start..y * w + xs.end] {
                    for c in 0..3 {
                        sum[c] += p[c];
                    }
                }
            }
            out.push(sum.map(|v| (linear_to_srgb(v / count) * 255.0 + 0.5) as u8));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thumbnail_averages_in_linear_and_encodes_srgb() {
        // Half black, half white columns average to linear 0.5 → sRGB 188.
        let mut img = Image::filled(4, [0.0, 0.0, 0.0, 1.0]);
        for (i, p) in img.pixels_mut().iter_mut().enumerate() {
            if i % 2 == 0 {
                *p = [1.0, 1.0, 1.0, 1.0];
            }
        }
        assert_eq!(thumbnail(&img, 1.into()), [[188, 188, 188]]);
        assert_eq!(
            thumbnail(&img, 8.into()).len(),
            64,
            "enlarging repeats pixels"
        );
    }

    #[test]
    fn a_non_square_recipe_keeps_its_aspect_in_its_tiles() {
        assert_eq!(tile_size(Size::new(256, 64), 128), Size::new(128, 32));
        assert_eq!(tile_size(Size::new(64, 256), 16), Size::new(4, 16));
        assert_eq!(tile_size(Size::new(1024, 1), 128), Size::new(128, 1));
        assert_eq!(tile_size(64.into(), 16), Size::new(16, 16));
        // A left half white, right half black 4×2 image: the 2×1 tile reads each.
        let img = Image::fill_uv(Size::new(4, 2), |u, _| [(u < 0.5) as u8 as f32; 4]);
        assert_eq!(thumbnail(&img, Size::new(2, 1)), [[255; 3], [0; 3]]);
    }
}
