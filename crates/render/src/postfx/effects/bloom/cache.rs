use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use crate::GpuTexture;
use rusty_asset::convert::texture_ref;
use rusty_asset::MapRef;
use rusty_components::is_render_texture;
use rusty_core::fs::freshness::Freshness;
use rusty_core::fs::written::Stale;

pub(crate) fn file(setting: &str) -> MapRef {
    match rusty_core::project::engine_relative(setting) {
        Some(relative) => rusty_core::project::engine_path(&relative).into(),
        None => setting.into(),
    }
}

struct Slot<T> {
    file: MapRef,
    texture: Option<T>,
}

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

    pub(crate) fn get(&self, setting: &str) -> Option<T> {
        self.loaded.get(setting)?.texture.clone()
    }

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
