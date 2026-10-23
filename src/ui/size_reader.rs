//! src/ui/size_reader.rs — filling the texture sizes the UI lays out with (#1221,
//! #1241).
//!
//! The layout measures an `Image` by its texture's size but never reads a file; a
//! [`TextureSizeReader`] fills the [`TextureSizes`] table it reads. The reader is
//! part of the tick: `GameWorld::tick` refreshes the `ScreenSize` resource's table
//! through it before stepping, so the window, the headless harness and any loop
//! that calls `tick` lay out alike, and no caller can forget it. Each size comes
//! from the file's **header**: no decode and no GPU. The header is the source's
//! size whatever the device or quality tier does with the upload, as Unity's
//! `Sprite.rect` is. The renderer draws with the same table.
//!
//! Only `Simple` and `Filled` Images of a file are read: a `Sliced` or `Tiled` one
//! measures by its borders, and a render texture is never on disk. Freshness is the
//! shared [`Freshness`]: a path the engine re-writes (`Texture.Bake` at a new
//! resolution) is re-read at the next refresh, and a file that could not be read
//! has no entry and is remembered as a miss, read again once its file appears or
//! the engine writes it, as the renderer's own texture cache does.
//!
//! **A refresh with nothing to do does nothing**: no write, no retry due and the
//! same measured paths as last time cost one walk of the Images, with no
//! allocation and no copy of the shared table.
//!
//! The table holds only paths the scene's Images measure by: a size set by hand
//! for any other path is dropped at the next refresh, and one set for a scene path
//! stays until the engine re-writes that file.

use std::collections::BTreeMap;
use std::rc::Rc;

use glam::Vec2;

use crate::asset::MapRef;
use crate::components::{ImageComponent, ImageType, RENDER_TEXTURE_PREFIX};
use crate::core::fs::freshness::Freshness;
use crate::core::fs::written::Stale;
use crate::ecs::World;
use crate::ui::TextureSizes;

/// The table's filler: its place in the write log, its misses, and the paths it
/// measured last. One per [`GameWorld`](crate::app::GameWorld).
#[derive(Debug, Default)]
pub struct TextureSizeReader {
    freshness: Freshness,
    /// Each path the scene's Images measure by, with the refresh it was last seen on.
    measured: BTreeMap<String, u64>,
    /// Refreshes so far.
    refreshes: u64,
}

impl TextureSizeReader {
    /// A reader that starts at the write log's current generation.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bring `sizes` up to date with `world`'s Images and the files on disk (see the
    /// module docs). Touches the shared table only when something changed.
    pub fn refresh(&mut self, world: &World, sizes: &mut Rc<TextureSizes>) {
        let changes = self.freshness.poll();
        let scene_changed = self.scan(world);
        // Every measured path has a size or is a miss, and the table holds nothing
        // else, unless someone edited it by hand.
        let in_step = sizes.len() + self.freshness.miss_count() == self.measured.len();
        if changes.stale.is_empty() && !scene_changed && in_step {
            return;
        }
        let stale = Stale::new(changes.stale);
        let is_stale = |p: &str| !stale.is_empty() && stale.matches(p);
        let measured = &self.measured;
        self.freshness.retain_misses(|p| measured.contains_key(p));
        self.freshness.forget_misses(&stale);
        let gone: Vec<String> = sizes
            .paths()
            .filter(|p| !measured.contains_key(*p) || is_stale(p))
            .map(str::to_string)
            .collect();
        let read: Vec<&String> = measured
            .keys()
            .filter(|p| sizes.get(p).is_none() || gone.contains(p))
            .filter(|p| !self.freshness.is_miss(p))
            .collect();
        if gone.is_empty() && read.is_empty() {
            return;
        }
        let table = Rc::make_mut(sizes);
        for path in &gone {
            table.remove(path);
        }
        for path in read {
            match image::image_dimensions(path) {
                Ok((w, h)) => table.set(path.clone(), Vec2::new(w as f32, h as f32)),
                Err(_) => self.freshness.miss(path),
            }
        }
    }

    /// Mark every path `world`'s Images measure by as seen this refresh. Returns
    /// whether the set changed: a path is new, or one is no longer used.
    fn scan(&mut self, world: &World) -> bool {
        self.refreshes += 1;
        let (now, measured) = (self.refreshes, &mut self.measured);
        let mut added = false;
        world.each_image(|img| {
            let Some(path) = measured_path(img) else {
                return;
            };
            match measured.get_mut(path) {
                Some(seen) => *seen = now,
                None => {
                    measured.insert(path.to_string(), now);
                    added = true;
                }
            }
        });
        let before = measured.len();
        measured.retain(|_, seen| *seen == now);
        added || measured.len() != before
    }
}

/// The texture path `img` measures by, if it measures by one on disk.
fn measured_path(img: &ImageComponent) -> Option<&str> {
    let by_texture = matches!(img.image_type, ImageType::Simple | ImageType::Filled);
    let path = img
        .texture
        .as_ref()
        .map(MapRef::as_str)
        .filter(|_| by_texture)?;
    (!path.starts_with(RENDER_TEXTURE_PREFIX)).then_some(path)
}

#[cfg(test)]
#[path = "size_reader_tests.rs"]
mod tests;
