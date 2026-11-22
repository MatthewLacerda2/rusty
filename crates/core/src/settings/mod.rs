pub mod audio;
pub mod decals;
pub mod demos;
pub mod graphics_options;
pub mod logs;
pub mod quality;
pub mod render_scale;
pub mod video;

use self::graphics_options::GraphicsOptions;
use self::quality::{QualityPresets, QualityValues};
use self::render_scale::{RenderScaleDefault, RenderScaleSetting};
use crate::storage::Storage;

pub const GRAPHICS_NAMESPACE: &str = "graphics";
pub const QUALITY_KEY: &str = "quality";

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GraphicsSettings {
    /// A copy of the running preset's values: whoever selects or edits a preset
    /// writes them here too.
    pub quality: QualityValues,
    pub options: GraphicsOptions,
    pub render_scale: RenderScaleSetting,
}

impl GraphicsSettings {
    pub fn load(
        storage: &Storage,
        (defaults, render_scale): (GraphicsOptions, RenderScaleDefault),
        presets: &QualityPresets,
    ) -> Self {
        Self {
            quality: presets.values(saved_quality(storage).as_deref()),
            options: GraphicsOptions::load(storage, defaults),
            render_scale: RenderScaleSetting::load(storage, render_scale),
        }
    }

    /// The preset's name is not stored here: it goes through [`store_quality`].
    pub fn store(&self, storage: &mut Storage) {
        self.options.store(storage);
        self.render_scale.store(storage);
    }
}

pub fn saved_quality(storage: &Storage) -> Option<String> {
    let value = storage.get(GRAPHICS_NAMESPACE, QUALITY_KEY)?;
    value.as_str().map(str::to_string)
}

pub fn store_quality(storage: &mut Storage, name: Option<&str>) {
    match name {
        Some(name) => {
            let name = serde_json::Value::String(name.to_string());
            storage.set(GRAPHICS_NAMESPACE, QUALITY_KEY, name);
        }
        None => {
            storage.delete(GRAPHICS_NAMESPACE, QUALITY_KEY);
        }
    }
}
