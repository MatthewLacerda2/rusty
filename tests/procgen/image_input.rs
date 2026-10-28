//! The `image` op (#1491): a decoded file feeds a recipe like any generator.

use rusty::procgen::recipe::{ImageEncoding, ImageTexture, OpKind, TextureRecipe};
use rusty::procgen::{evaluate, evaluate_with, RunError, Source, Sources};

use crate::procgen::node;

/// A 4×4 PNG whose texel `(x, y)` holds R = 17·(4y + x), G = 255 − R, B = 85,
/// A = 255, encoded as bytes.
fn swatch() -> Vec<u8> {
    let png = image::RgbaImage::from_fn(4, 4, |x, y| {
        let r = (17 * (4 * y + x)) as u8;
        image::Rgba([r, 255 - r, 85, 255])
    });
    let mut bytes = std::io::Cursor::new(Vec::new());
    png.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    bytes.into_inner()
}

fn inverted(encoding: ImageEncoding) -> TextureRecipe {
    let image = OpKind::Image(ImageTexture {
        path: "textures/swatch.png".into(),
        encoding,
    });
    TextureRecipe::new(
        4,
        vec![
            node("photo", image, &[]),
            node("inv", OpKind::Invert, &["photo"]),
        ],
    )
}

fn sources() -> Sources {
    let mut sources = Sources::new();
    sources.insert(
        "textures/swatch.png",
        Source::decode("textures/swatch.png", &swatch()).unwrap(),
    );
    sources
}

/// The inverted texels for each byte decoded by `decode`.
fn expected(decode: impl Fn(u8) -> f32) -> Vec<[f32; 4]> {
    (0..16u32)
        .map(|i| {
            let r = (17 * i) as u8;
            [
                1.0 - decode(r),
                1.0 - decode(255 - r),
                1.0 - decode(85),
                1.0,
            ]
        })
        .collect()
}

fn assert_close(got: &[[f32; 4]], want: &[[f32; 4]]) {
    for (g, w) in got.iter().zip(want) {
        for c in 0..4 {
            assert!((g[c] - w[c]).abs() < 1e-5, "{g:?} vs {w:?}");
        }
    }
}

#[test]
fn an_inverted_image_is_srgb_decoded_by_default() {
    let recipe: TextureRecipe = serde_json::from_str(
        r#"{ "resolution": 4, "nodes": [
            { "id": "photo", "op": "image", "path": "textures/swatch.png" },
            { "id": "inv", "op": "invert", "inputs": ["photo"] } ] }"#,
    )
    .unwrap();
    assert_eq!(recipe, inverted(ImageEncoding::Srgb));
    let img = evaluate_with(&recipe, &sources()).unwrap();
    assert_close(img.pixels(), &expected(rusty::core::color::srgb_to_linear));
}

#[test]
fn an_inverted_image_reads_raw_with_linear() {
    let img = evaluate_with(&inverted(ImageEncoding::Linear), &sources()).unwrap();
    assert_close(img.pixels(), &expected(|b| f32::from(b) / 255.0));
}

#[test]
fn an_image_without_its_file_is_an_error_naming_the_path() {
    let err = evaluate(&inverted(ImageEncoding::Srgb)).unwrap_err();
    assert_eq!(
        err,
        RunError::MissingSource {
            node: "photo".into(),
            path: "textures/swatch.png".into()
        }
    );
    assert!(err.to_string().contains("textures/swatch.png"));
}
