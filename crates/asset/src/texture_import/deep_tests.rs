//! A normal map kept at 16 bits is never block-compressed (BC5 would round it to 8 bits), and a compressed copy made before it was 16-bit is an orphan.

use std::path::Path;

use image::{DynamicImage, ImageBuffer, Rgba};

use super::orphans::orphans;
use super::*;
use crate::sidecar::set_texture;

/// A `side`² PNG at `dir/name` of a slope too gentle for 8 bits: x climbs
/// 0.5 → 0.5 + 1/512 across it, a half 8-bit step; 16-bit when `deep`.
fn slope(dir: &Path, name: &str, side: u32, deep: bool) -> std::path::PathBuf {
    let path = dir.join(name);
    let x = |i: u32| 32768 + (i * 128 / side) as u16;
    let img = ImageBuffer::from_fn(side, side, |i, _| Rgba([x(i), 32768, 65535, 65535]));
    let img = DynamicImage::ImageRgba16(img);
    match deep {
        true => img.save(&path).unwrap(),
        false => img.to_rgba8().save(&path).unwrap(),
    }
    path
}

fn normal_map(max_size: u32) -> TextureSettings {
    TextureSettings {
        texture_type: TextureType::NormalMap,
        max_size,
        ..Default::default()
    }
}

#[test]
fn only_a_16_bit_normal_map_is_deep() {
    let dir = rusty_test_support::temp::dir();
    let deep = slope(&dir, "deep.png", 8, true);
    let flat = slope(&dir, "flat.png", 8, false);
    assert!(is_deep_normal(&deep, &normal_map(2048)));
    assert!(!is_deep_normal(&flat, &normal_map(2048)), "an 8-bit file");
    let colour = TextureSettings::default();
    assert!(!is_deep_normal(&deep, &colour), "a Default texture");
    assert!(!is_deep_normal(&dir.join("absent.png"), &normal_map(2048)));
}

#[test]
fn a_16_bit_normal_map_is_never_compressed_and_fits_at_16_bits() {
    let dir = rusty_test_support::temp::dir();
    let deep = slope(&dir, "ripple.png", 64, true);
    set_texture(&deep, normal_map(32)).unwrap();
    assert_eq!(refresh(&deep, ""), Ok(false), "no BC5 copy is made");
    let loaded = load(&deep, &normal_map(32), TextureKind::Normal, true).unwrap();
    let Texels::Image(img) = loaded.texels else {
        panic!("uploaded uncompressed");
    };
    assert_eq!((img.width(), img.color()), (32, image::ColorType::Rgba16));
    let row: std::collections::BTreeSet<u16> = (0..32)
        .map(|i| img.to_rgba16().get_pixel(i, 0).0[0])
        .collect();
    assert!(row.len() > 16, "the fit keeps the slope: {row:?}");
}

#[test]
fn a_copy_made_while_8_bit_is_an_orphan_once_16_bit() {
    let dir = rusty_test_support::temp::dir();
    let (cache, sources) = (dir.join("cache"), dir.join("assets"));
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::create_dir_all(&sources).unwrap();
    for (name, deep) in [("tile.png", true), ("brick.png", false)] {
        let path = slope(&sources, name, 8, deep);
        set_texture(&path, normal_map(2048)).unwrap();
        std::fs::write(cache.join(format!("{name}{CACHE_SUFFIX}")), b"").unwrap();
    }
    assert_eq!(orphans(&cache, &sources), [cache.join("tile.png.bctex")]);
}

#[test]
fn a_16_bit_data_image_fits_and_encodes_as_its_8_bit_twin() {
    let dir = rusty_test_support::temp::dir();
    let speckle = |x: u32, y: u32| ((x * 7919 + y * 104_729) % 65_536) as u16;
    let img = ImageBuffer::from_fn(64, 64, |x, y| {
        Rgba([speckle(x, y), speckle(y, x), 0, 65535])
    });
    let deep = DynamicImage::ImageRgba16(img);
    let data = TextureSettings {
        srgb: false,
        max_size: 32,
        ..Default::default()
    };
    let (deep_path, flat_path) = (dir.join("data16.png"), dir.join("data8.png"));
    deep.save(&deep_path).unwrap();
    deep.to_rgba8().save(&flat_path).unwrap();
    for path in [&deep_path, &flat_path] {
        set_texture(path, data).unwrap();
        assert_eq!(refresh(path, ""), Ok(true), "{}", path.display());
    }
    let blocks = |path: &Path| match load(path, &data, TextureKind::Data, true).unwrap().texels {
        Texels::Blocks(blocks) => blocks,
        Texels::Image(_) => panic!("{} has no cache", path.display()),
    };
    assert_eq!(blocks(&deep_path), blocks(&flat_path), "fitted at 8 bits");
    let fitted = fit(deep.clone(), 32, TextureKind::Data);
    assert_eq!(
        fitted,
        fit(
            DynamicImage::ImageRgba8(deep.to_rgba8()),
            32,
            TextureKind::Data
        )
    );
}

#[test]
fn only_a_normal_map_reads_as_kept_at_16_bits() {
    use image::ColorType;
    assert!(keeps_deep(&normal_map(2048), ColorType::Rgba16));
    assert!(keeps_deep(&normal_map(2048), ColorType::Rgb32F));
    assert!(!keeps_deep(&normal_map(2048), ColorType::Rgba8));
    assert!(
        !keeps_deep(&TextureSettings::default(), ColorType::Rgba16),
        "Default"
    );
    let single = TextureSettings {
        texture_type: TextureType::SingleChannel,
        ..Default::default()
    };
    assert!(!keeps_deep(&single, ColorType::L16));
}
