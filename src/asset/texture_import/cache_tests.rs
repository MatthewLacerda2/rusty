//! The import cache of compressed textures (#1574): a hit, a miss on a changed
//! stamp or a bad file, and *Max Size* applied before compressing.

use image::{Rgba, RgbaImage};

use super::*;
use crate::asset::convert;

fn settings(compression: Compression) -> TextureSettings {
    TextureSettings {
        compression,
        ..Default::default()
    }
}

fn gradient(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, y| Rgba([(x * 2) as u8, (y * 2) as u8, 128, 255]))
}

#[test]
fn a_compressed_texture_is_cached_and_re_made_when_its_settings_change() {
    let dir = crate::test_temp::dir();
    let png = dir.join("floor.png");
    gradient(16, 16).save(&png).unwrap();
    let normal = settings(Compression::Normal);
    let first = compressed(&png, &normal, TextureKind::Colour).unwrap();
    let cached = convert::cache_location_as(&png, ".colour.bctex").unwrap();
    assert_eq!(
        cache::read(
            &cached,
            cache::stamp(&std::fs::read(&png).unwrap(), &normal, TextureKind::Colour)
        ),
        Some(first.clone())
    );
    assert_eq!(
        compressed(&png, &normal, TextureKind::Colour),
        Some(first),
        "a hit"
    );
    let high = compressed(
        &png,
        &settings(Compression::HighQuality),
        TextureKind::Colour,
    );
    assert_eq!(
        high.unwrap().format,
        BlockFormat::Bc7,
        "a new stamp re-encodes"
    );
    std::fs::write(&cached, b"RBCT garbage").unwrap();
    assert!(
        compressed(&png, &normal, TextureKind::Colour).is_some(),
        "a bad file misses"
    );
    assert!(compressed(&png, &settings(Compression::None), TextureKind::Colour).is_none());
}

#[test]
fn max_size_is_applied_before_compressing() {
    let dir = crate::test_temp::dir();
    let png = dir.join("big.png");
    gradient(128, 64).save(&png).unwrap();
    let small = TextureSettings {
        max_size: 32,
        ..settings(Compression::Normal)
    };
    let made = compressed(&png, &small, TextureKind::Data).unwrap();
    assert_eq!((made.width, made.height), (32, 16));
}
