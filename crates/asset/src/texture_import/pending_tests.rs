//! [`pending`]: the images an import would encode now, read without encoding:
//! one with no cache is listed, one whose cache is current is not, and one an
//! import leaves uncompressed (a size not a multiple of 4, a 16-bit normal map)
//! never is, nor does the memory tests' blank cache (`cache::write_blank`) make
//! one for it.

use image::{DynamicImage, ImageBuffer, Rgba, RgbaImage};

use super::super::cache_file;
use super::*;

fn flat(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_pixel(w, h, Rgba([90, 120, 30, 255]))
}

#[test]
fn pending_lists_what_an_import_would_encode_and_nothing_else() {
    let dir = rusty_test_support::temp::dir();
    let (floor, odd) = (dir.join("floor.png"), dir.join("odd.png"));
    flat(16, 16).save(&floor).unwrap();
    flat(10, 6).save(&odd).unwrap();
    assert_eq!(
        pending(&dir),
        std::slice::from_ref(&floor),
        "uncached, the odd one aside"
    );
    assert!(!cache_file(&floor).unwrap().exists(), "nothing was encoded");
    assert_eq!(refresh(&floor, ""), Ok(true));
    assert!(pending(&dir).is_empty(), "current once imported");
}

#[test]
fn a_blank_cache_is_refused_where_an_import_makes_none() {
    let dir = rusty_test_support::temp::dir();
    let (floor, odd) = (dir.join("floor.png"), dir.join("odd.png"));
    flat(16, 16).save(&floor).unwrap();
    flat(10, 6).save(&odd).unwrap();
    let settings = crate::sidecar::texture(&odd);
    let refused = super::super::write_blank(&odd, &settings).unwrap_err();
    assert!(refused.contains("multiple of 4"), "{refused}");
    assert!(!cache_file(&odd).unwrap().exists());
    assert_eq!(super::super::write_blank(&floor, &settings), Ok(()));
    assert!(pending(&dir).is_empty(), "the blank cache is current");
}

#[test]
fn a_blank_cache_is_refused_for_a_16_bit_normal_map() {
    use crate::sidecar::{set_texture, TextureSettings, TextureType};
    let dir = rusty_test_support::temp::dir();
    let deep = dir.join("deep.png");
    let img = ImageBuffer::from_pixel(16, 16, Rgba([32768u16, 32768, 65535, 65535]));
    DynamicImage::ImageRgba16(img).save(&deep).unwrap();
    let settings = TextureSettings {
        texture_type: TextureType::NormalMap,
        ..Default::default()
    };
    set_texture(&deep, settings).unwrap();
    assert!(pending(&dir).is_empty(), "an import leaves it uncompressed");
    let refused = super::super::write_blank(&deep, &settings).unwrap_err();
    assert!(refused.contains("16-bit"), "{refused}");
    assert!(!cache_file(&deep).unwrap().exists());
}
