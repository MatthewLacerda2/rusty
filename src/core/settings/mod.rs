//! The engine's global settings resources: the quality presets' values, the player's
//! graphics options, the video settings and the game's decal budgets.

pub mod decals;
pub mod graphics_options;
pub mod quality;
pub mod render_scale;
pub mod video;

use self::graphics_options::GraphicsOptions;
use self::quality::{QualityPresets, QualityValues};
use self::render_scale::RenderScale;
use crate::core::storage::Storage;

/// The `Storage` namespace the graphics settings persist under: the selected quality
/// preset's name (key [`QUALITY_KEY`]), the player's options
/// ([`OPTIONS_KEY`](graphics_options::OPTIONS_KEY)) and the render scale
/// ([`RENDER_SCALE_KEY`](render_scale::RENDER_SCALE_KEY)).
pub const GRAPHICS_NAMESPACE: &str = "graphics";
/// The selected quality preset's `Storage` key.
pub const QUALITY_KEY: &str = "quality";

/// What the renderer's post-FX chain reads besides the camera: the running quality
/// preset's values (#1522), the player's options (#1438) and the scene's render scale
/// (#1498). One shared cell carries it from the `Graphics` namespace (and the editor
/// header) to the renderer each frame. The preset's *name* and the list live with
/// the project (`Application::quality`); whoever selects or edits a preset writes
/// its values here.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GraphicsSettings {
    pub quality: QualityValues,
    pub options: GraphicsOptions,
    pub render_scale: RenderScale,
}

impl GraphicsSettings {
    /// What startup reads: the values of the saved preset among `presets` (their
    /// initial one when none is saved, or the saved one is gone), the player's
    /// saved options over the project's `defaults`, and the saved render scale.
    pub fn load(storage: &Storage, defaults: GraphicsOptions, presets: &QualityPresets) -> Self {
        Self {
            quality: presets.values(saved_quality(storage).as_deref()),
            options: GraphicsOptions::load(storage, defaults),
            render_scale: RenderScale::load(storage),
        }
    }

    /// Write the player's options and the render scale into `storage`; the Stop and
    /// quit flushes put them on disk. The preset's name goes through
    /// [`store_quality`].
    pub fn store(&self, storage: &mut Storage) {
        self.options.store(storage);
        self.render_scale.store(storage);
    }
}

/// The name of the quality preset the player last selected, if any.
pub fn saved_quality(storage: &Storage) -> Option<String> {
    let value = storage.get(GRAPHICS_NAMESPACE, QUALITY_KEY)?;
    value.as_str().map(str::to_string)
}

/// Remember `name` as the player's quality preset (`None` forgets it).
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
