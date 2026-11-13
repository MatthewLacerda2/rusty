//! *sRGB (Color Texture)* off (#1574): a packed metallic/roughness map a model's
//! import names is marked linear, so its compressed mips average its raw values,
//! as an uncompressed upload in that slot does (#1322), never in linear light.

use block_compression::{decode::decompress_blocks_as_rgba8, BC7Settings, CompressionVariant};
use image::{Rgba, RgbaImage};

use super::*;

/// The first texel of mip 1 of `png`'s cache, decoded.
fn first_mip_texel(png: &Path) -> [u8; 4] {
    let settings = crate::sidecar::texture(png);
    let Texels::Blocks(made) = load(png, &settings, TextureKind::Mask, true)
        .unwrap()
        .texels
    else {
        panic!("{} has no current cache", png.display());
    };
    let mut decoded = vec![0; 4 * 4 * 4];
    let bc7 = CompressionVariant::BC7(BC7Settings::opaque_very_fast());
    decompress_blocks_as_rgba8(bc7, 4, 4, &made.levels[1], &mut decoded);
    [decoded[0], decoded[1], decoded[2], decoded[3]]
}

#[test]
fn an_imported_packed_orm_keeps_raw_averaged_mips_through_the_cache() {
    let dir = rusty_test_support::temp::dir();
    // A 0/255 roughness (green) and metallic (blue) checker.
    let checker = RgbaImage::from_fn(8, 8, |x, y| {
        let v = ((x + y) % 2 * 255) as u8;
        Rgba([255, v, v, 255])
    });
    let orm = dir.join("orm.png");
    checker.save(&orm).unwrap();
    // What a model's material import does to the map it names in that slot
    // (`scene::authoring::defaults`, whose own test holds that it does).
    crate::sidecar::fit_data_slot(&orm);
    let settings = crate::sidecar::texture(&orm);
    assert!(is_linear(&settings), "the data slot turned sRGB off");
    assert_eq!(refresh(&orm, ""), Ok(true));
    let texel = first_mip_texel(&orm);
    assert!(texel[1].abs_diff(128) <= 3, "raw average: {texel:?}");

    // The same checker as colour averages in linear light (sRGB 188).
    let colour = dir.join("colour.png");
    checker.save(&colour).unwrap();
    assert_eq!(refresh(&colour, ""), Ok(true));
    let texel = first_mip_texel(&colour);
    assert!(texel[1].abs_diff(188) <= 3, "linear light: {texel:?}");
}

#[test]
fn srgb_off_reads_a_colour_slot_as_data_and_only_for_default() {
    let off = TextureSettings {
        srgb: false,
        ..Default::default()
    };
    assert_eq!(TextureKind::Colour.reading(&off), TextureKind::Data);
    assert_eq!(TextureKind::Mask.reading(&off), TextureKind::Mask);
    let on = TextureSettings::default();
    assert_eq!(TextureKind::Colour.reading(&on), TextureKind::Colour);
    let normal_off = TextureSettings {
        texture_type: TextureType::NormalMap,
        ..off
    };
    assert!(!is_linear(&normal_off), "read for Default only");
}
