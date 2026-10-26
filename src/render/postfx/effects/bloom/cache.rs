//! The lens dirt textures the volumes name (#1442), cached by the path as written
//! (the setting) and kept in step with the disk the way the Color Lookup's LUTs are
//! (`grading::color_lookup::user`): a written file is read again, a missing one is
//! retried once it appears, and one that exists but can't be decoded stays `None`
//! until it is written again. A failure warns once per path, until it loads. The
//! cache is its own, so a bad dirt never drops the materials' bind groups.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use crate::asset::convert::texture_ref;
use crate::asset::MapRef;
use crate::components::is_render_texture;
use crate::components::rendering::post_processing::DEFAULT_DIRT_TEXTURE;
use crate::core::fs::freshness::Freshness;
use crate::core::fs::written::Stale;
use crate::render::GpuTexture;

/// The file a dirt `setting` reads: [`DEFAULT_DIRT_TEXTURE`] is the engine's own,
/// wherever `engine/` is; any other path as it is. Resolved only when it is read.
pub(crate) fn file(setting: &str) -> MapRef {
    if setting == DEFAULT_DIRT_TEXTURE {
        crate::core::project::engine_path("textures/lens_dirt.png").into()
    } else {
        setting.into()
    }
}

/// One setting's read: the file it resolved to, and its upload (`None`: failed).
struct Slot<T> {
    file: MapRef,
    texture: Option<T>,
}

/// Every dirt texture a volume has named, by setting. `T` is the upload (a
/// [`GpuTexture`]; tests use a plain value).
pub(crate) struct DirtCache<T = Rc<GpuTexture>> {
    freshness: Freshness,
    loaded: BTreeMap<String, Slot<T>>,
    warned: BTreeSet<String>,
}

impl<T> Default for DirtCache<T> {
    fn default() -> Self {
        Self {
            freshness: Freshness::new(),
            loaded: BTreeMap::new(),
            warned: BTreeSet::new(),
        }
    }
}

impl<T: Clone> DirtCache<T> {
    /// Read `setting` with `upload` unless it was read already. A render texture
    /// (`rt:`) is never read here: the frame that draws it registers it.
    pub(crate) fn request(&mut self, setting: &str, upload: impl FnOnce(&MapRef) -> Option<T>) {
        if is_render_texture(setting) || self.loaded.contains_key(setting) {
            return;
        }
        let file = file(setting);
        let texture = upload(&file);
        if texture.is_some() {
            self.warned.remove(setting);
        } else {
            let absent = !texture_ref::exists(file.as_str());
            if absent {
                self.freshness.miss(file.as_str());
            }
            if self.warned.insert(setting.to_string()) {
                let why = if absent {
                    "not found"
                } else {
                    "not a readable image"
                };
                log::warn!("[Render] Bloom dirt texture `{setting}` ignored: {why}");
            }
        }
        self.loaded
            .insert(setting.to_string(), Slot { file, texture });
    }

    /// The upload `setting` names, when it loaded.
    pub(crate) fn get(&self, setting: &str) -> Option<T> {
        self.loaded.get(setting)?.texture.clone()
    }

    /// Drop what was written, or has appeared, since the last look. Called once
    /// per frame, before any request.
    pub(crate) fn refresh(&mut self) {
        let changes = self.freshness.poll_with(texture_ref::exists);
        if changes.stale.is_empty() {
            return;
        }
        let stale = Stale::new(changes.stale);
        let is_stale = |file: &str| texture_ref::is_written(&stale, file);
        self.loaded.retain(|_, slot| !is_stale(slot.file.as_str()));
        self.freshness.forget_misses_where(is_stale);
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
