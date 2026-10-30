use block_compression::{decode::decompress_blocks_as_rgba8, BC7Settings, CompressionVariant};
use image::{DynamicImage, Rgba, RgbaImage};

use super::encode::{encode, Cores};
use super::shrink::fitted_size;
use super::*;

fn settings(compression: Compression) -> TextureSettings {
    TextureSettings {
        compression,
        ..Default::default()
    }
}

fn typed(texture_type: TextureType) -> TextureSettings {
    TextureSettings {
        texture_type,
        ..Default::default()
    }
}

/// A `w`×`h` image of smooth, varied colour; `alpha` its every texel's alpha.
fn gradient(w: u32, h: u32, alpha: u8) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, y| {
        Rgba([(x * 8) as u8, (y * 8) as u8, 128, alpha])
    })
}

#[test]
fn each_texture_type_compresses_to_its_format_whatever_the_texels() {
    let image = DynamicImage::ImageRgba8(gradient(8, 8, 100));
    let cases = [
        (TextureType::Default, BlockFormat::Bc7),
        (TextureType::NormalMap, BlockFormat::Bc5),
        (TextureType::SingleChannel, BlockFormat::Bc4),
    ];
    for (texture_type, want) in cases {
        assert_eq!(format_of(&typed(texture_type)), Some(want));
        let made = encode(&image, &typed(texture_type), Cores::All).unwrap();
        assert_eq!(made.format, want, "{texture_type:?}");
    }
    let none = settings(Compression::None);
    assert_eq!(format_of(&none), None);
    assert!(encode(&image, &none, Cores::All).is_none());
}

#[test]
fn a_type_fits_only_the_slots_that_read_all_its_channels() {
    use TextureKind::*;
    for kind in [Colour, Data, Normal, Mask] {
        assert!(fits(TextureType::Default, kind));
        assert_eq!(fits(TextureType::NormalMap, kind), kind == Normal);
        assert_eq!(fits(TextureType::SingleChannel, kind), kind == Mask);
    }
}

/// Two contrasting colours in one block (red over blue) decode as both, not as
/// their average: the case BC1 and BC7's ultra-fast profiles get wrong.
#[test]
fn a_block_of_two_contrasting_colours_keeps_both() {
    let block = RgbaImage::from_fn(4, 4, |_, y| match y < 2 {
        true => Rgba([220, 30, 30, 255]),
        false => Rgba([30, 30, 220, 255]),
    });
    let image = DynamicImage::ImageRgba8(block.clone());
    let made = encode(&image, &settings(Compression::Normal), Cores::All).unwrap();
    let mut decoded = vec![0; 64];
    let bc7 = CompressionVariant::BC7(BC7Settings::opaque_very_fast());
    decompress_blocks_as_rgba8(bc7, 4, 4, &made.levels[0], &mut decoded);
    let worst = block
        .as_raw()
        .iter()
        .zip(&decoded)
        .map(|(a, b)| a.abs_diff(*b))
        .max();
    assert!(worst.unwrap() <= 4, "off by {worst:?}: {decoded:?}");
}

#[test]
fn a_chain_has_every_level_in_whole_blocks_and_odd_sides_are_left_uncompressed() {
    let image = DynamicImage::ImageRgba8(gradient(64, 16, 255));
    let made = encode(&image, &settings(Compression::Normal), Cores::All).unwrap();
    assert_eq!((made.width, made.height, made.levels.len()), (64, 16, 7));
    for (i, level) in made.levels.iter().enumerate() {
        let (w, h) = ((64 >> i).max(1), (16 >> i).max(1));
        assert_eq!(level.len(), BlockFormat::Bc7.level_bytes(w, h), "level {i}");
    }
    let no_mips = TextureSettings {
        generate_mipmaps: false,
        ..settings(Compression::Normal)
    };
    assert_eq!(
        encode(&image, &no_mips, Cores::All).unwrap().levels.len(),
        1
    );
    let odd = DynamicImage::ImageRgba8(gradient(30, 16, 255));
    assert!(encode(&odd, &settings(Compression::Normal), Cores::All).is_none());
}

#[test]
fn the_blocks_decode_close_to_the_source() {
    let source = gradient(32, 32, 200);
    let image = DynamicImage::ImageRgba8(source.clone());
    let made = encode(&image, &settings(Compression::HighQuality), Cores::All).unwrap();
    assert_eq!(made.format, BlockFormat::Bc7);
    let mut decoded = vec![0; 32 * 32 * 4];
    let bc7 = CompressionVariant::BC7(BC7Settings::alpha_fast());
    decompress_blocks_as_rgba8(bc7, 32, 32, &made.levels[0], &mut decoded);
    let worst = source
        .as_raw()
        .iter()
        .zip(&decoded)
        .map(|(a, b)| a.abs_diff(*b))
        .max();
    assert!(worst.unwrap() <= 8, "BC7 off by {worst:?}");
}

#[test]
fn fit_keeps_the_aspect_and_colour_averages_in_linear_light() {
    let wide = DynamicImage::ImageRgba8(gradient(300, 100, 255));
    let fitted = fit(wide.clone(), 128, TextureKind::Data);
    assert_eq!((fitted.width(), fitted.height()), (128, 44), "rounded to 4");
    assert_eq!(
        fit(wide, 512, TextureKind::Data).width(),
        300,
        "fits already"
    );
    // A black and white checker: 50% linear light is sRGB 188, not byte 128.
    let checker = RgbaImage::from_fn(64, 64, |x, y| Rgba([((x + y) % 2 * 255) as u8; 4]));
    let checker = DynamicImage::ImageRgba8(checker);
    let colour = fit(checker.clone(), 32, TextureKind::Colour).to_rgba8();
    let data = fit(checker, 32, TextureKind::Data).to_rgba8();
    assert!(
        colour.get_pixel(5, 5)[0].abs_diff(188) <= 2,
        "{:?}",
        colour.get_pixel(5, 5)
    );
    assert!(
        data.get_pixel(5, 5)[0].abs_diff(128) <= 2,
        "{:?}",
        data.get_pixel(5, 5)
    );
}

#[test]
fn a_downscaled_side_stays_a_multiple_of_4() {
    assert_eq!(fitted_size(4096, 2732, 2048), (2048, 1368));
    assert_eq!(fitted_size(8192, 4, 32), (32, 4), "at least one block");
    assert_eq!(
        fitted_size(30, 18, 64),
        (30, 18),
        "an image that fits is kept"
    );
}
