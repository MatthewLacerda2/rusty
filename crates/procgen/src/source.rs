//! procgen is sim code and reads no files: the caller decodes them and hands them in.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rusty_font::{FontData, FontHandle};

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

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn texels(&self) -> &[Rgba] {
        &self.texels
    }
}

pub type Loader = Arc<dyn Fn(&str) -> Result<Source, String> + Send + Sync>;

#[derive(Clone, Default)]
pub struct Sources {
    decoded: BTreeMap<String, Source>,
    lazy: BTreeSet<String>,
    loader: Option<Loader>,
    fonts: BTreeMap<String, FontHandle>,
}

impl std::fmt::Debug for Sources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sources")
            .field("decoded", &self.decoded.keys().collect::<Vec<_>>())
            .field("lazy", &self.lazy)
            .field("fonts", &self.fonts.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Sources {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn lazy(paths: impl IntoIterator<Item = String>, loader: Loader) -> Self {
        Self {
            decoded: BTreeMap::new(),
            lazy: paths.into_iter().collect(),
            loader: Some(loader),
            fonts: BTreeMap::new(),
        }
    }

    pub fn insert_font(
        &mut self,
        path: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Result<FontHandle, String> {
        let font = FontData::parse(bytes.into())
            .ok_or("not a .ttf / .otf font this can read (truncated, or another format)")?;
        let font = FontHandle::new(font);
        self.insert_parsed_font(path, font.clone());
        Ok(font)
    }

    pub fn insert_parsed_font(&mut self, path: impl Into<String>, font: FontHandle) {
        self.fonts.insert(path.into(), font);
    }

    pub fn font(&self, path: &str) -> Option<FontHandle> {
        self.fonts.get(path).cloned()
    }

    pub fn insert(&mut self, path: impl Into<String>, source: Source) {
        self.decoded.insert(path.into(), source);
    }

    pub fn get(&self, path: &str) -> Option<&Source> {
        self.decoded.get(path)
    }

    pub fn remove(&mut self, path: &str) {
        self.decoded.remove(path);
    }

    pub fn has(&self, path: &str) -> bool {
        self.decoded.contains_key(path) || self.lazy.contains(path)
    }

    pub fn load(&self, path: &str) -> Option<Result<Source, String>> {
        let loader = self.loader.as_ref()?;
        (self.lazy.contains(path) && !self.decoded.contains_key(path)).then(|| loader(path))
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
    fn a_lazy_path_is_here_but_decoded_only_on_load() {
        let loader: Loader = Arc::new(|path| match path {
            "a.png" => Source::from_texels(1, 1, vec![[1.0; 4]]),
            _ => Err("no such file".into()),
        });
        let sources = Sources::lazy(["a.png".into(), "b.png".into()], loader);
        assert!(sources.has("a.png") && sources.get("a.png").is_none());
        assert_eq!(sources.load("a.png").unwrap().unwrap().texels(), [[1.0; 4]]);
        assert!(sources.load("b.png").unwrap().is_err());
        assert!(!sources.has("c.png") && sources.load("c.png").is_none());
    }

    #[test]
    fn a_font_is_parsed_when_it_is_handed_in() {
        let mut sources = Sources::new();
        assert!(sources
            .insert_font("x.ttf", b"not a font".to_vec())
            .is_err());
        assert!(sources.font("x.ttf").is_none());
    }

    #[test]
    fn bytes_that_are_no_image_are_an_error() {
        assert!(Source::decode("x.png", b"not a png").is_err());
        assert!(Source::decode("x", b"not a png").is_err());
    }

    #[test]
    fn a_tga_decodes_by_its_extension() {
        // TGA has no magic bytes: guessing from the bytes alone fails.
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
