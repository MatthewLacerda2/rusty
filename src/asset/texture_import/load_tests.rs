//! Which file a load and a refresh read (#1574): a TGA, which has no signature to
//! guess it by, decodes by its extension; and whether a cache is current is found
//! from its head and level lengths alone, agreeing with a full read.

use image::{Rgba, RgbaImage};

use super::*;

fn gradient(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, y| Rgba([(x * 8) as u8, (y * 8) as u8, 64, 255]))
}

#[test]
fn a_tga_loads_and_compresses() {
    let dir = crate::test_temp::dir();
    let tga = dir.join("crate.tga");
    gradient(8, 4).save(&tga).unwrap();
    let settings = TextureSettings::default();
    let plain = load(&tga, &settings, TextureKind::Colour, false).unwrap();
    assert!(matches!(plain.texels, Texels::Image(_)));
    assert_eq!(plain.source_size, (8, 4));
    assert_eq!(refresh(&tga, ""), Ok(true));
    let made = load(&tga, &settings, TextureKind::Colour, true).unwrap();
    assert!(
        matches!(made.texels, Texels::Blocks(_)),
        "its cache is read"
    );
    assert_eq!(made.source_size, (8, 4));
}

#[test]
fn a_cache_is_current_exactly_when_a_read_takes_it() {
    let dir = crate::test_temp::dir();
    let png = dir.join("floor.png");
    gradient(8, 8).save(&png).unwrap();
    refresh(&png, "").unwrap();
    let out = cache_file(&png).unwrap();
    let settings = crate::asset::sidecar::texture(&png);
    let digest = crate::core::hash::content(&std::fs::read(&png).unwrap());
    let stamp = cache::stamp(digest, &settings);
    assert!(cache::is_current(&out, stamp));
    assert!(!cache::is_current(&out, stamp ^ 1), "another stamp");
    let good = std::fs::read(&out).unwrap();
    let at = |offset: usize, value: u32| {
        let mut bad = good.clone();
        bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        bad
    };
    let mut longer = good.clone();
    longer.push(0);
    let cases = [
        at(25, 15),
        at(21, 9),
        at(13, 6),
        at(21, 0),
        good[..good.len() - 1].to_vec(),
        longer,
    ];
    for bad in cases {
        std::fs::write(&out, &bad).unwrap();
        assert!(cache::read(&out, stamp).is_none());
        assert!(!cache::is_current(&out, stamp));
    }
}
