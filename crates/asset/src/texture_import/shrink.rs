//! Colour averages in linear light, as the GPU mip pass does (`render::gpu::textures::mips`), so
//! downscaling never darkens a high-contrast texture; every other kind averages raw values.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use image::imageops::{resize, FilterType};
use image::{DynamicImage, Rgba32FImage, RgbaImage};

use super::TextureKind;
use crate::sidecar::MAX_SIZES;
use rusty_core::color::{linear_to_srgb, srgb_to_linear};

/// An image of more than 8 bits a channel fitted for a normal slot stays 16-bit, so a normal map
/// kept at 16 bits keeps its slopes; every other kind is fitted at 8 bits.
pub fn fit(image: DynamicImage, max_size: u32, kind: TextureKind) -> DynamicImage {
    let (w, h) = fitted_size(image.width(), image.height(), max_size);
    if (w, h) == (image.width(), image.height()) {
        return image;
    }
    if kind == TextureKind::Normal && super::is_deep(image.color()) {
        let resized = resize(&image.to_rgba32f(), w, h, FilterType::Triangle);
        return DynamicImage::ImageRgba16(DynamicImage::ImageRgba32F(resized).to_rgba16());
    }
    let working = to_working(&image.to_rgba8(), kind);
    DynamicImage::ImageRgba8(to_bytes(
        &resize(&working, w, h, FilterType::Triangle),
        kind,
    ))
}

/// A downscaled side is rounded to a multiple of 4, at least 4, so it stays compressible.
pub fn fitted_size(width: u32, height: u32, max_size: u32) -> (u32, u32) {
    let longer = width.max(height);
    if longer <= max_size {
        return (width, height);
    }
    let scale = |side: u32| {
        let exact = u64::from(side) * u64::from(max_size) / u64::from(longer);
        ((exact + 2) / 4 * 4).max(4) as u32
    };
    (scale(width), scale(height))
}

type Warned = BTreeSet<(PathBuf, (u32, u32), u32)>;
static WARNED: Mutex<Warned> = Mutex::new(BTreeSet::new());

/// Once per file, size and Max Size, so a changed file or setting is told again.
pub(super) fn warn_if_shrunk(source: &Path, size: (u32, u32), max_size: u32) {
    let (w, h) = size;
    let fitted = fitted_size(w, h, max_size);
    if fitted == size {
        return;
    }
    let mut warned = WARNED.lock().unwrap_or_else(|e| e.into_inner());
    if !warned.insert((source.to_path_buf(), size, max_size)) {
        return;
    }
    let largest = MAX_SIZES[MAX_SIZES.len() - 1];
    let advice = match fitted_size(w, h, largest) == size {
        true => "raise Max Size in its .meta (Assets.SetTextureImportSettings) to keep its size",
        false => "no Max Size holds it, so save the file smaller to keep its detail",
    };
    log::warn!(
        "[Import] {}: {w}x{h} is larger than its Max Size {max_size}, so it is \
         imported at {}x{}; {advice}",
        source.display(),
        fitted.0,
        fitted.1,
    );
}

pub(super) fn half(level: &Rgba32FImage) -> Rgba32FImage {
    let (w, h) = ((level.width() / 2).max(1), (level.height() / 2).max(1));
    resize(level, w, h, FilterType::Triangle)
}

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
