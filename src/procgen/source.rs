//! src/procgen/source.rs — the decoded image files a recipe's `image` nodes read
//! (#1491).
//!
//! procgen is sim code and reads no files: the caller resolves each path
//! [`TextureRecipe::image_paths`](super::TextureRecipe::image_paths) names, reads
//! the bytes, and hands them here to decode. [`Sources`] maps each path, as the
//! recipe spells it, to its [`Source`]; the runner refuses a recipe whose `image`
//! node has none, before anything evaluates.

use std::collections::BTreeMap;

use super::image_buf::Rgba;

/// One decoded image file: its size and its texels, row-major, each channel the
/// stored value scaled to `[0, 1]` and nothing else (no colour transfer: the
/// `image` node's `encoding` decides that). 8- and 16-bit files keep their full
/// precision.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    width: u32,
    height: u32,
    texels: Vec<Rgba>,
}

impl Source {
    /// Decode an encoded image (any format the `image` crate reads: png, jpeg,
    /// tga, …) from its bytes. The format comes from `path`'s extension when it
    /// names one, since TGA has no magic bytes to guess from; otherwise from the
    /// bytes.
    pub fn decode(path: &str, bytes: &[u8]) -> Result<Source, String> {
        let decoded = match image::ImageFormat::from_path(path) {
            Ok(format) => image::load_from_memory_with_format(bytes, format),
            Err(_) => image::load_from_memory(bytes),
        }
        .map_err(|e| e.to_string())?;
        let rgba = decoded.to_rgba32f();
        let (width, height) = rgba.dimensions();
        let texels = rgba.pixels().map(|p| p.0).collect();
        Self::from_texels(width, height, texels)
    }

    /// A source from raw `[0, 1]` texels, row-major; refused when empty or when the
    /// count is not `width × height`.
    pub fn from_texels(width: u32, height: u32, texels: Vec<Rgba>) -> Result<Source, String> {
        if width == 0 || height == 0 {
            return Err("the image is empty".into());
        }
        if texels.len() != width as usize * height as usize {
            return Err(format!(
                "{} texels for a {width}×{height} image",
                texels.len()
            ));
        }
        Ok(Self {
            width,
            height,
            texels,
        })
    }

    /// The image's `(width, height)` in texels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The texels, row-major.
    pub fn texels(&self) -> &[Rgba] {
        &self.texels
    }
}

/// The decoded files of a recipe's `image` nodes, keyed by path as the recipe
/// writes it. Empty by default: a recipe without `image` nodes needs none.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sources(BTreeMap<String, Source>);

impl Sources {
    /// No sources.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add (or replace) the source for `path`.
    pub fn insert(&mut self, path: impl Into<String>, source: Source) {
        self.0.insert(path.into(), source);
    }

    /// The source for `path`, if the caller provided one.
    pub fn get(&self, path: &str) -> Option<&Source> {
        self.0.get(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_size_that_does_not_match_the_texels_is_refused() {
        assert!(Source::from_texels(2, 2, vec![[0.0; 4]; 3]).is_err());
        assert!(Source::from_texels(0, 0, vec![]).is_err());
        assert!(Source::from_texels(2, 1, vec![[0.0; 4]; 2]).is_ok());
    }

    #[test]
    fn a_16_bit_png_decodes_at_full_precision() {
        let mut png = image::ImageBuffer::<image::Rgba<u16>, _>::new(2, 1);
        png.put_pixel(1, 0, image::Rgba([1, 32768, 65535, 65535]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        png.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let source = Source::decode("ramp.png", bytes.get_ref()).unwrap();
        assert_eq!(source.size(), (2, 1));
        let [r, g, b, a] = source.texels()[1];
        assert_eq!((r, g, b, a), (1.0 / 65535.0, 32768.0 / 65535.0, 1.0, 1.0));
    }

    #[test]
    fn bytes_that_are_no_image_are_an_error() {
        assert!(Source::decode("x.png", b"not a png").is_err());
        assert!(Source::decode("x", b"not a png").is_err());
    }

    #[test]
    fn a_tga_decodes_by_its_extension() {
        // TGA has no magic bytes: guessing from the bytes alone fails (#1491 review).
        let mut tga = image::RgbaImage::new(2, 1);
        tga.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        tga.write_to(&mut bytes, image::ImageFormat::Tga).unwrap();
        let source = Source::decode("textures/crate.TGA", bytes.get_ref()).unwrap();
        assert_eq!(
            source.texels(),
            [[1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 0.0]]
        );
    }
}
