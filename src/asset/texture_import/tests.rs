use block_compression::{decode::decompress_blocks_as_rgba8, BC7Settings, CompressionVariant};
use image::{DynamicImage, Rgba, RgbaImage};

use super::encode::{encode, format_for};
use super::*;

fn settings(compression: Compression) -> TextureSettings {
    TextureSettings {
        compression,
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
fn each_kind_compresses_to_the_format_unitys_table_names() {
    let (normal, high) = (Compression::Normal, Compression::HighQuality);
    let (opaque, clear) = (gradient(8, 8, 255), gradient(8, 8, 100));
    let grey = RgbaImage::from_fn(8, 8, |x, _| {
        Rgba([x as u8 * 30, x as u8 * 30, x as u8 * 30, 255])
    });
    let cases = [
        (&opaque, normal, TextureKind::Colour, BlockFormat::Bc1),
        (&clear, normal, TextureKind::Colour, BlockFormat::Bc7),
        (&opaque, high, TextureKind::Colour, BlockFormat::Bc7),
        (&opaque, normal, TextureKind::Data, BlockFormat::Bc1),
        (&opaque, normal, TextureKind::Normal, BlockFormat::Bc5),
        (&grey, high, TextureKind::Mask, BlockFormat::Bc4),
        (&opaque, normal, TextureKind::Mask, BlockFormat::Bc1),
        (&grey, normal, TextureKind::Data, BlockFormat::Bc1),
    ];
    for (texels, compression, kind, want) in cases {
        let got = format_for(texels, compression, kind);
        assert_eq!(got, want, "{kind:?} at {compression:?}");
    }
}

#[test]
fn a_chain_has_every_level_in_whole_blocks_and_odd_sides_are_left_uncompressed() {
    let image = DynamicImage::ImageRgba8(gradient(64, 16, 255));
    let made = encode(&image, &settings(Compression::Normal), TextureKind::Colour).unwrap();
    assert_eq!((made.width, made.height, made.levels.len()), (64, 16, 7));
    for (i, level) in made.levels.iter().enumerate() {
        let (w, h) = ((64 >> i).max(1), (16 >> i).max(1));
        assert_eq!(level.len(), BlockFormat::Bc1.level_bytes(w, h), "level {i}");
    }
    let no_mips = TextureSettings {
        generate_mipmaps: false,
        ..settings(Compression::Normal)
    };
    assert_eq!(
        encode(&image, &no_mips, TextureKind::Data)
            .unwrap()
            .levels
            .len(),
        1
    );
    let odd = DynamicImage::ImageRgba8(gradient(30, 16, 255));
    assert!(encode(&odd, &settings(Compression::Normal), TextureKind::Colour).is_none());
}

#[test]
fn the_blocks_decode_close_to_the_source() {
    let source = gradient(32, 32, 200);
    let image = DynamicImage::ImageRgba8(source.clone());
    let made = encode(
        &image,
        &settings(Compression::HighQuality),
        TextureKind::Colour,
    )
    .unwrap();
    assert_eq!(made.format, BlockFormat::Bc7);
    let mut decoded = vec![0; 32 * 32 * 4];
    let bc7 = CompressionVariant::BC7(BC7Settings::alpha_basic());
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
    assert_eq!((fitted.width(), fitted.height()), (128, 42));
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
