//! src/render/gpu/textures/sizes.rs — the texture sizes the UI lays out with (#1221).
//!
//! The platform half of `ui::TextureSizes`. The layout measures an `Image` by its
//! texture's size but never reads a file (it is sim); a [`TextureSizeReader`] fills
//! the table it reads **before every tick**, from each texture file's **header** —
//! no decode and no GPU, so the window shell and the headless harness run the same
//! code and lay out alike. The header is the source's size whatever the device or
//! quality tier does with the upload, as Unity's `Sprite.rect` is. The renderer
//! draws with the same table (`Renderer::set_ui_textures`).
//!
//! Only `Simple` and `Filled` Images of a file are read: a `Sliced` or `Tiled` one
//! measures by its borders, and a render texture is never on disk. It keeps to the
//! write-log rule (`core::fs::written`): a path the engine re-writes (`Texture.Bake`
//! at a new resolution) is re-read at the next refresh. A file that cannot be read
//! has no entry and is remembered as a *miss*, re-tried every
//! [`MISS_RETRY_FRAMES`] refreshes (and at once when the engine writes it), as the
//! renderer's own texture cache does.
//!
//! The table holds only paths the scene's Images measure by: a size set by hand
//! for any other path is dropped at the next refresh, and one set for a scene path
//! stays until the engine re-writes that file.

use std::collections::BTreeSet;
use std::rc::Rc;

use glam::Vec2;

use super::freshness::MISS_RETRY_FRAMES;
use crate::components::ImageType;
use crate::core::fs::written::{written_since, Stale};
use crate::ecs::World;
use crate::render::render_texture::is_render_texture;
use crate::ui::{ScreenSize, TextureSizes};

/// The platform's side of the table: where it is in the write log, and which
/// paths could not be read. One per frame loop (the shell, the harness).
pub struct TextureSizeReader {
    /// The newest write generation already applied.
    seen: u64,
    /// Paths whose header could not be read, re-tried every `MISS_RETRY_FRAMES`.
    misses: BTreeSet<String>,
    /// Refreshes since the misses were last re-tried.
    since_retry: u64,
}

impl Default for TextureSizeReader {
    fn default() -> Self {
        Self::new()
    }
}

impl TextureSizeReader {
    /// A reader that starts at the write log's current generation, as every
    /// write-log consumer does.
    pub fn new() -> Self {
        Self {
            seen: written_since(u64::MAX).0,
            misses: BTreeSet::new(),
            since_retry: 0,
        }
    }

    /// Bring `sizes` up to date with `world`'s Images and the files on disk (see the
    /// module docs). Touches the shared table only when something changed.
    pub fn refresh(&mut self, world: &World, sizes: &mut Rc<TextureSizes>) {
        let (seen, written) = written_since(self.seen);
        self.seen = seen;
        let stale = Stale::new(written);
        let is_stale = |p: &str| !stale.is_empty() && stale.matches(p);
        self.since_retry += 1;
        let retry = self.since_retry >= MISS_RETRY_FRAMES;
        if retry {
            self.since_retry = 0;
        }
        let paths = measured_paths(world);
        self.misses
            .retain(|p| !retry && paths.contains(p) && !is_stale(p));
        let gone: Vec<String> = sizes
            .paths()
            .filter(|p| !paths.contains(*p) || is_stale(p))
            .map(str::to_string)
            .collect();
        let read: Vec<&String> = paths
            .iter()
            .filter(|p| sizes.get(p).is_none() || gone.contains(p))
            .filter(|p| !self.misses.contains(*p))
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
                Err(_) => {
                    self.misses.insert(path.clone());
                }
            }
        }
    }
}

impl TextureSizeReader {
    /// [`refresh`](Self::refresh) the table `screen` shares with every view (the
    /// sim's `ScreenSize` resource), and return it.
    pub fn refresh_screen(&mut self, world: &World, screen: &mut ScreenSize) -> Rc<TextureSizes> {
        let mut textures = Rc::clone(screen.textures());
        // Drop the resource's handle first, so an unshared table is updated in place.
        screen.set_textures(Rc::default());
        self.refresh(world, &mut textures);
        screen.set_textures(Rc::clone(&textures));
        textures
    }
}

/// Every texture path a `Simple` or `Filled` Image in `world` measures by.
fn measured_paths(world: &World) -> BTreeSet<String> {
    world
        .ids_with_image()
        .into_iter()
        .filter_map(|id| {
            let img = world.image(id)?;
            let by_texture = matches!(img.image_type, ImageType::Simple | ImageType::Filled);
            img.texture.clone().filter(|_| by_texture)
        })
        .filter(|path| !is_render_texture(path))
        .collect()
}

#[cfg(test)]
#[path = "sizes_tests.rs"]
mod sizes_tests;
