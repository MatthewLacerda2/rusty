//! Every extension `asset::IMAGE_EXTENSIONS` offers decodes (#994): the list is a
//! promise the `image` crate's compiled-in features have to keep.

use crate::asset::IMAGE_EXTENSIONS;

#[test]
fn every_listed_image_format_round_trips_through_the_decoder() {
    let dir = std::env::temp_dir().join(format!("rusty_image_formats_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pixels = image::RgbImage::from_pixel(4, 4, image::Rgb([200, 40, 10]));
    for ext in IMAGE_EXTENSIONS {
        let path = dir.join(format!("swatch.{}", ext.to_uppercase()));
        pixels
            .save_with_format(&path, image::ImageFormat::from_extension(ext).unwrap())
            .unwrap_or_else(|e| panic!("encode .{ext}: {e}"));
        let decoded = image::open(&path).unwrap_or_else(|e| panic!("decode .{ext}: {e}"));
        assert_eq!((decoded.width(), decoded.height()), (4, 4), "{ext}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
