//! A contact sheet of every node of a recipe (#1486), the PNG `Texture.Preview`
//! writes.
//!
//! The agent's stand-in for the live thumbnails Blender draws on each node: one
//! image with every node's output shrunk to a `size`² tile, in `recipe.nodes` order,
//! left to right then top to bottom, in a near-square grid. No text is drawn; the
//! caller maps tile `k` to the `k`th node. Each tile is taken through
//! [`super::evaluate_each`] as its node finishes, so only the small tiles are held.
//!
//! Every tile is sRGB-encoded, data included, the way Blender's previews pass
//! everything through the view transform: a 0.5 mask shows as 188, not 128. Alpha
//! is not shown; the sheet is opaque.

use image::{ImageBuffer, Rgba};

use super::evaluate_each;
use crate::core::color::linear_to_srgb;
use crate::procgen::image_buf::Image;
use crate::procgen::recipe::TextureRecipe;
use crate::procgen::source::Sources;

/// A rendered contact sheet: the encoded pixels and the grid's column count.
pub struct Sheet {
    pub pixels: ImageBuffer<Rgba<u8>, Vec<u8>>,
    pub columns: u32,
}

/// Render every node of `recipe`, its `image` nodes reading `sources`, to a
/// `size`²-tile contact sheet. A `size` above
/// the recipe's resolution is refused: a tile larger than the image adds nothing,
/// and an unbounded one could not be allocated.
pub fn sheet(recipe: &TextureRecipe, sources: &Sources, size: u32) -> Result<Sheet, String> {
    let size = size.max(1);
    if size > recipe.resolution {
        return Err(format!(
            "size {size} is larger than the recipe's resolution {}",
            recipe.resolution
        ));
    }
    let n = recipe.nodes.len() as u32;
    let columns = (1..=n).find(|c| c * c >= n).unwrap_or(1);
    let rows = n.div_ceil(columns);
    let too_big = || format!("a {columns}×{rows} sheet of {size}² tiles is too large");
    let width = columns.checked_mul(size).ok_or_else(too_big)?;
    let height = rows.checked_mul(size).ok_or_else(too_big)?;
    let mut tiles: Vec<Option<Vec<[u8; 3]>>> = recipe.nodes.iter().map(|_| None).collect();
    evaluate_each(recipe, sources, |i, img| {
        tiles[i] = Some(thumbnail(img, size))
    })
    .map_err(|e| e.to_string())?;
    let mut pixels = ImageBuffer::from_pixel(width, height, Rgba([0, 0, 0, 255]));
    for (k, tile) in tiles.into_iter().enumerate() {
        let tile = tile.expect("evaluate_each visits every node");
        let (ox, oy) = ((k as u32 % columns) * size, (k as u32 / columns) * size);
        for (j, [r, g, b]) in tile.into_iter().enumerate() {
            let (x, y) = (j as u32 % size, j as u32 / size);
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

/// `img` resampled to `size`², sRGB-encoded: each tile pixel is the linear mean of
/// the source pixels it covers (a box filter), or the nearest one when enlarging.
pub fn thumbnail(img: &Image, size: u32) -> Vec<[u8; 3]> {
    let res = img.resolution() as usize;
    let size = size as usize;
    let span = |t: usize| {
        let start = t * res / size;
        start..((t + 1) * res / size).max(start + 1)
    };
    let mut out = Vec::with_capacity(size * size);
    for ty in 0..size {
        for tx in 0..size {
            let mut sum = [0.0f32; 3];
            let (ys, xs) = (span(ty), span(tx));
            let count = (ys.len() * xs.len()) as f32;
            for y in ys {
                for p in &img.pixels()[y * res + xs.start..y * res + xs.end] {
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
        assert_eq!(thumbnail(&img, 1), [[188, 188, 188]]);
        assert_eq!(thumbnail(&img, 8).len(), 64, "enlarging repeats pixels");
    }
}
