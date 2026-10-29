//! Downscaling an image: to its *Max Size* ([`fit`]) and one mip level at a time
//! ([`half`]). Colour averages in linear light, as the GPU mip pass does
//! (`render::gpu::textures::mips`), so neither darkens a high-contrast texture;
//! every other kind averages its raw values.

use image::imageops::{resize, FilterType};
use image::{DynamicImage, Rgba32FImage, RgbaImage};

use super::TextureKind;
use crate::core::color::{linear_to_srgb, srgb_to_linear};

/// `image` downscaled, its aspect kept, until its longer side is at most
/// `max_size` (Unity's *Max Size*); unchanged when it already fits.
pub fn fit(image: DynamicImage, max_size: u32, kind: TextureKind) -> DynamicImage {
    let (width, height) = (image.width(), image.height());
    let longer = width.max(height);
    if longer <= max_size {
        return image;
    }
    let scale = |side: u32| ((u64::from(side) * u64::from(max_size)) / u64::from(longer)).max(1);
    let (w, h) = (scale(width) as u32, scale(height) as u32);
    let working = to_working(&image.to_rgba8(), kind);
    DynamicImage::ImageRgba8(to_bytes(
        &resize(&working, w, h, FilterType::Triangle),
        kind,
    ))
}

/// The next mip level of `level`: half its size on each side, at least 1.
pub(super) fn half(level: &Rgba32FImage) -> Rgba32FImage {
    let (w, h) = ((level.width() / 2).max(1), (level.height() / 2).max(1));
    resize(level, w, h, FilterType::Triangle)
}

/// `image` as the values its mips average: linear light for colour (alpha stays
/// linear), the raw bytes over 255 for the rest.
pub(super) fn to_working(image: &RgbaImage, kind: TextureKind) -> Rgba32FImage {
    let colour = kind.is_colour();
    Rgba32FImage::from_fn(image.width(), image.height(), |x, y| {
        let p = image.get_pixel(x, y).0;
        let c = |b: u8| match colour {
            true => srgb_to_linear(b),
            false => f32::from(b) / 255.0,
        };
        image::Rgba([c(p[0]), c(p[1]), c(p[2]), f32::from(p[3]) / 255.0])
    })
}

/// The inverse of [`to_working`], rounded to the nearest byte.
pub(super) fn to_bytes(image: &Rgba32FImage, kind: TextureKind) -> RgbaImage {
    let colour = kind.is_colour();
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    RgbaImage::from_fn(image.width(), image.height(), |x, y| {
        let p = image.get_pixel(x, y).0;
        let c = |v: f32| match colour {
            true => byte(linear_to_srgb(v)),
            false => byte(v),
        };
        image::Rgba([c(p[0]), c(p[1]), c(p[2]), byte(p[3])])
    })
}
