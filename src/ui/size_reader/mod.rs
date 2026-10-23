//! src/ui/size_reader/mod.rs — filling the texture sizes the UI lays out with (#1221,
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
//! measures by its borders, and a render texture is never on disk. A texture an
//! import extracted (`model.fbx#tex/Skin`) is read where the import cache holds
//! it ([`MapRef::file`], #1256), and goes stale with its source. Freshness is the
//! shared [`Freshness`]: a path the engine re-writes (`Texture.Bake` at a new
//! resolution) is re-read at the next refresh, and a file that could not be read
//! has no entry and is remembered as a miss, read again once its file appears or
//! the engine writes it, as the renderer's own texture cache does.
//!
//! **A refresh with nothing to do does nothing**: no write, no retry due, the
//! same measured paths as last time and no hand edit of the table since
//! ([`TextureSizes::edits`]) cost one walk of the Images, with no allocation and
//! no copy of the shared table. Headers are read before the table is touched, so
//! a retried file that still cannot be read copies nothing either.
//!
//! The table holds only paths the scene's Images measure by: a size set by hand
//! for any other path is dropped at the next refresh, and one set for a scene path
//! stays until the engine re-writes that file.

use std::collections::BTreeMap;
use std::rc::Rc;

use glam::Vec2;

use crate::asset::convert::texture_ref;
use crate::asset::MapRef;
use crate::components::{is_render_texture, ImageComponent, ImageType};
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
    measured: BTreeMap<MapRef, u64>,
    /// Refreshes so far.
    refreshes: u64,
    /// The table's [`TextureSizes::edits`] as the last refresh left it: any other
    /// count means someone edited it by hand since. `None` before the first.
    left_at: Option<u64>,
    /// Refreshes that went past the idle check (tests).
    #[cfg(test)]
    sweeps: u64,
}

impl TextureSizeReader {
    /// A reader that starts at the write log's current generation.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bring `sizes` up to date with `world`'s Images and the files on disk (see the
    /// module docs). Touches the shared table only when something changed.
    pub fn refresh(&mut self, world: &World, sizes: &mut Rc<TextureSizes>) {
        let changes = self.freshness.poll_with(texture_ref::exists);
        let scene_changed = self.scan(world);
        let in_step = self.left_at == Some(sizes.edits());
        if changes.stale.is_empty() && !scene_changed && in_step {
            return;
        }
        #[cfg(test)]
        {
            self.sweeps += 1;
        }
        self.sweep(Stale::new(changes.stale), sizes);
        self.left_at = Some(sizes.edits());
    }

    /// Drop what went stale or is no longer measured, then read every measured
    /// header the table lacks. The headers are read before the table is touched,
    /// so a refresh whose reads all fail copies nothing.
    fn sweep(&mut self, stale: Stale, sizes: &mut Rc<TextureSizes>) {
        let measured = &self.measured;
        let is_stale = |p: &str| texture_ref::is_written(&stale, p);
        self.freshness
            .retain_misses(|p| measured.contains_key(p.as_str()));
        self.freshness.forget_misses_where(is_stale);
        let gone: Vec<String> = sizes
            .paths()
            .filter(|p| !measured.contains_key(*p) || is_stale(p))
            .map(str::to_string)
            .collect();
        let mut read = Vec::new();
        for map in measured.keys() {
            let held = sizes.get(map.as_str()).is_some() && !gone.iter().any(|g| map.as_str() == g);
            if held || self.freshness.is_miss(map.as_str()) {
                continue;
            }
            match header(map) {
                Some(size) => read.push((map.as_str(), size)),
                None => self.freshness.miss(map.as_str()),
            }
        }
        if gone.is_empty() && read.is_empty() {
            return;
        }
        let table = Rc::make_mut(sizes);
        for path in &gone {
            table.remove(path);
        }
        for (path, size) in read {
            table.set(path, size);
        }
    }

    /// Mark every path `world`'s Images measure by as seen this refresh. Returns
    /// whether the set changed: a path is new, or one is no longer used.
    fn scan(&mut self, world: &World) -> bool {
        self.refreshes += 1;
        let (now, measured) = (self.refreshes, &mut self.measured);
        let mut added = false;
        world.each_image(|img| {
            let Some(map) = measured_map(img) else {
                return;
            };
            match measured.get_mut(map) {
                Some(seen) => *seen = now,
                None => {
                    measured.insert(map.clone(), now);
                    added = true;
                }
            }
        });
        let before = measured.len();
        measured.retain(|_, seen| *seen == now);
        added || measured.len() != before
    }
}

/// The texture `img` measures by, if it measures by one on disk.
fn measured_map(img: &ImageComponent) -> Option<&MapRef> {
    let by_texture = matches!(img.image_type, ImageType::Simple | ImageType::Filled);
    let map = img.texture.as_ref().filter(|_| by_texture)?;
    (!is_render_texture(map.as_str())).then_some(map)
}

/// The size in texels `map`'s file header gives, a texture an import extracted
/// read where the import cache holds it (#1256).
fn header(map: &MapRef) -> Option<Vec2> {
    let (w, h) = image::image_dimensions(map.file()?).ok()?;
    Some(Vec2::new(w as f32, h as f32))
}

#[cfg(test)]
mod idle_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tick_tests;
