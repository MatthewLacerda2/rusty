//! src/ui/layout/native.rs — texture native sizes for layout (#421).
//!
//! An `Image`'s preferred size is its texture's size in texels (one texel is one
//! reference unit, as the renderer draws it). The sim has no GPU textures, so the
//! size is read from the file's header once and cached per path — like fonts
//! (`ui::text::font`). A missing or unreadable file measures zero.

use crate::core::collections::Map;
use std::sync::{Mutex, OnceLock};

use glam::Vec2;

type Cache = Mutex<Map<String, Vec2>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Map::default()))
}

/// The texture at `path`'s size in texels, zero when it cannot be read.
pub(super) fn texture_size(path: &str) -> Vec2 {
    let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
    *cache.entry(path.to_string()).or_insert_with(|| {
        image::image_dimensions(path).map_or(Vec2::ZERO, |(w, h)| Vec2::new(w as f32, h as f32))
    })
}
