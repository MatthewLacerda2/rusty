//! The import cache of compressed textures (#1574): made by a refresh and only
//! read by a load, re-made when the settings change, a malformed file a miss, and
//! *Max Size* applied before compressing.

use image::{Rgba, RgbaImage};

use super::*;
use crate::asset::sidecar::set_texture;

fn gradient(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, y| Rgba([(x * 2) as u8, (y * 2) as u8, 128, 255]))
}

/// What a load of `png` finds for a colour slot, compressed if it can be.
fn blocks(png: &Path) -> Option<Compressed> {
    let settings = crate::asset::sidecar::texture(png);
    match load(png, &settings, TextureKind::Colour, true)?.texels {
        Texels::Blocks(blocks) => Some(blocks),
        Texels::Image(_) => None,
    }
}

#[test]
fn a_load_reads_what_a_refresh_made_and_never_encodes() {
    let dir = crate::test_temp::dir();
    let png = dir.join("floor.png");
    gradient(16, 16).save(&png).unwrap();
    assert!(
        blocks(&png).is_none(),
        "no cache: uncompressed, nothing encoded"
    );
    assert!(!cache_file(&png).unwrap().exists());
    assert_eq!(refresh(&png, ""), Ok(true));
    assert_eq!(refresh(&png, ""), Ok(false), "current");
    let made = blocks(&png).unwrap();
    assert_eq!(
        (made.format, made.width, made.levels.len()),
        (BlockFormat::Bc7, 16, 5)
    );
    let normal = TextureSettings {
        texture_type: TextureType::NormalMap,
        ..Default::default()
    };
    set_texture(&png, normal).unwrap();
    let settings = crate::asset::sidecar::texture(&png);
    let loaded = load(&png, &settings, TextureKind::Normal, true).unwrap();
    assert!(
        matches!(loaded.texels, Texels::Image(_)),
        "stale: uncompressed"
    );
    assert_eq!(refresh(&png, ""), Ok(true), "a new stamp re-encodes");
    let loaded = load(&png, &settings, TextureKind::Normal, true).unwrap();
    let Texels::Blocks(made) = loaded.texels else {
        panic!("re-made");
    };
    assert_eq!(made.format, BlockFormat::Bc5);
    assert_eq!(loaded.source_size, (16, 16));
}

#[test]
fn a_cache_whose_levels_do_not_match_its_header_is_a_miss() {
    let dir = crate::test_temp::dir();
    let png = dir.join("wall.png");
    gradient(8, 8).save(&png).unwrap();
    refresh(&png, "").unwrap();
    let out = cache_file(&png).unwrap();
    let good = std::fs::read(&out).unwrap();
    // The header: magic, stamp, format, width, height, count, then level 0's length.
    let at = |offset: usize, value: u32| {
        let mut bad = good.clone();
        bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        bad
    };
    for (what, bad) in [
        ("a level too short", at(25, 15)),
        ("too many levels", at(21, 9)),
        ("a side not in whole blocks", at(13, 6)),
        ("no levels", at(21, 0)),
    ] {
        std::fs::write(&out, bad).unwrap();
        assert!(blocks(&png).is_none(), "{what}");
    }
    std::fs::write(&out, &good[..good.len() - 1]).unwrap();
    assert!(blocks(&png).is_none(), "cut short");
    assert_eq!(refresh(&png, ""), Ok(true), "a bad cache is re-made");
}

#[test]
fn max_size_is_applied_before_compressing_and_an_odd_image_is_left_alone() {
    let dir = crate::test_temp::dir();
    let png = dir.join("big.png");
    gradient(128, 64).save(&png).unwrap();
    let small = TextureSettings {
        max_size: 32,
        ..Default::default()
    };
    set_texture(&png, small).unwrap();
    refresh(&png, "").unwrap();
    let made = blocks(&png).unwrap();
    assert_eq!((made.width, made.height), (32, 16));
    let odd = dir.join("odd.png");
    gradient(30, 18).save(&odd).unwrap();
    assert_eq!(refresh(&odd, ""), Ok(false));
    assert!(!cache_file(&odd).unwrap().exists(), "left uncompressed");
}

#[test]
fn refresh_all_makes_every_image_under_a_folder() {
    let dir = crate::test_temp::dir();
    let (a, b) = (dir.join("a.png"), dir.join("sub").join("b.png"));
    for path in [&a, &b] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        gradient(8, 8).save(path).unwrap();
    }
    std::fs::write(dir.join("notes.txt"), "not an image").unwrap();
    assert_eq!(images(&dir), [a, b]);
    assert_eq!(refresh_all(&dir).encoded, 2);
    assert_eq!(refresh_all(&dir), Refreshed::default());
    let broken = dir.join("broken.png");
    std::fs::write(&broken, b"not a png").unwrap();
    let done = refresh_all(&dir);
    assert_eq!(done.encoded, 0);
    assert_eq!(done.failed.len(), 1);
    assert_eq!(done.failed[0].0, broken);
}
