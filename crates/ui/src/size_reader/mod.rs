use std::collections::BTreeMap;
use std::rc::Rc;

use glam::Vec2;

use crate::TextureSizes;
use rusty_asset::convert::texture_ref;
use rusty_asset::MapRef;
use rusty_components::{is_render_texture, ImageComponent, ImageType};
use rusty_core::fs::freshness::Freshness;
use rusty_core::fs::written::Stale;
use rusty_ecs::World;

#[derive(Debug, Default)]
pub struct TextureSizeReader {
    freshness: Freshness,
    measured: BTreeMap<MapRef, u64>,
    /// Refreshes so far.
    refreshes: u64,
    left_at: Option<u64>,
    /// Refreshes that went past the idle check (tests).
    #[cfg(test)]
    sweeps: u64,
}

impl TextureSizeReader {
    #[cfg(test)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Bring `sizes` up to date with `world`'s Images and the files on disk (see the
    /// module docs). Touches the shared table only when something changed.
    pub fn refresh(&mut self, world: &World, sizes: &mut Rc<TextureSizes>) {
        let changes = self.freshness.poll_with(texture_ref::exists);
        let scene_changed = self.scan(world);
        let in_step = self.left_at == Some(sizes.edits());
        let stale = Stale::new(changes.stale);
        if !scene_changed && in_step && !self.concerns(&stale) {
            return;
        }
        #[cfg(test)]
        {
            self.sweeps += 1;
        }
        self.sweep(stale, sizes);
        self.left_at = Some(sizes.edits());
    }

    fn concerns(&self, stale: &Stale) -> bool {
        !stale.is_empty()
            && self
                .measured
                .keys()
                .any(|map| texture_ref::is_written(stale, map.as_str()))
    }

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
/// read where the import cache holds it.
fn header(map: &MapRef) -> Option<Vec2> {
    let (w, h) = image::image_dimensions(map.file()?).ok()?;
    Some(Vec2::new(w as f32, h as f32))
}

#[cfg(test)]
mod idle_tests;
#[cfg(test)]
mod tests;
