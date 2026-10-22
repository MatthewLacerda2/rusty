//! src/render/gpu/textures/sizes.rs — the texture sizes the UI lays out with (#1221).
//!
//! The platform half of `ui::TextureSizes`. The layout measures an `Image` by its
//! texture's size but never reads a file (it is sim); this fills the table it
//! reads, once per frame, from each texture file's **header** — no decode and no
//! GPU, so the window, the renderer's own UI pass and the headless harness all
//! run it and agree. The header is the source's size whatever the device or
//! quality tier does with the upload, as Unity's `Sprite.rect` is.
//!
//! It keeps to the write-log rule (`core::fs::written`): a path the engine
//! re-writes (`Texture.Bake` at a new resolution) is dropped and re-read the same
//! refresh, and a file that cannot be read has no entry and is tried again next
//! frame, so an Image laid out before its bake picks the size up once the file
//! exists. Only the paths the scene's Images use are kept.

use std::collections::BTreeSet;

use glam::Vec2;

use crate::core::fs::written::{written_since, Stale};
use crate::ecs::World;
use crate::render::render_texture::is_render_texture;
use crate::ui::TextureSizes;

/// Bring `sizes` up to date with `world`'s Images and the files on disk (see the
/// module docs).
pub fn refresh_texture_sizes(world: &World, sizes: &mut TextureSizes) {
    let (generation, written) = written_since(sizes.generation());
    sizes.set_generation(generation);
    let stale = Stale::new(written);
    let paths: BTreeSet<String> = world
        .ids_with_image()
        .into_iter()
        .filter_map(|id| world.image(id)?.texture.clone())
        .filter(|path| !is_render_texture(path))
        .collect();
    sizes.retain(|path| paths.contains(path) && (stale.is_empty() || !stale.matches(path)));
    for path in paths {
        if sizes.get(&path).is_some() {
            continue;
        }
        if let Ok((w, h)) = image::image_dimensions(&path) {
            sizes.set(path, Vec2::new(w as f32, h as f32));
        }
    }
}

#[cfg(test)]
#[path = "sizes_tests.rs"]
mod sizes_tests;
