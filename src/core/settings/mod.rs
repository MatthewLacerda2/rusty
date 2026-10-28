//! The engine's global settings resources: the quality preset, the player's
//! graphics options, the video settings and the game's decal budgets.

pub mod decals;
pub mod graphics_options;
pub mod quality;
pub mod render_scale;
pub mod video;

use self::graphics_options::GraphicsOptions;
use self::quality::QualityPreset;
use self::render_scale::RenderScale;
use crate::core::storage::Storage;

/// The `Storage` namespace the graphics settings persist under: the quality tier
/// (key [`QUALITY_KEY`]), the player's options
/// ([`OPTIONS_KEY`](graphics_options::OPTIONS_KEY)) and the render scale
/// ([`RENDER_SCALE_KEY`](render_scale::RENDER_SCALE_KEY)).
pub const GRAPHICS_NAMESPACE: &str = "graphics";
/// The quality tier's `Storage` key.
pub const QUALITY_KEY: &str = "quality";

/// What the renderer's post-FX chain is gated by besides the camera: the quality
/// tier and the player's options (#1438), and the scene's render scale (#1498). One
/// shared cell carries it from the `Graphics` namespace (and the editor header) to
/// the renderer each frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GraphicsSettings {
    pub quality: QualityPreset,
    pub options: GraphicsOptions,
    pub render_scale: RenderScale,
}

impl From<QualityPreset> for GraphicsSettings {
    /// `quality` with every player option allowed.
    fn from(quality: QualityPreset) -> Self {
        Self {
            quality,
            options: GraphicsOptions::default(),
            render_scale: RenderScale::NATIVE,
        }
    }
}

impl GraphicsSettings {
    /// The saved tier (the default tier when none), the player's saved options
    /// over the project's `defaults` and the saved render scale: what startup reads.
    pub fn load(storage: &Storage, defaults: GraphicsOptions) -> Self {
        let quality = storage
            .get(GRAPHICS_NAMESPACE, QUALITY_KEY)
            .and_then(|v| v.as_str().and_then(QualityPreset::parse))
            .unwrap_or_default();
        Self {
            quality,
            options: GraphicsOptions::load(storage, defaults),
            render_scale: RenderScale::load(storage),
        }
    }

    /// Write the tier, the player's options and the render scale into `storage`;
    /// the Stop and quit flushes put them on disk.
    pub fn store(&self, storage: &mut Storage) {
        let quality = serde_json::Value::String(self.quality.name().to_string());
        storage.set(GRAPHICS_NAMESPACE, QUALITY_KEY, quality);
        self.options.store(storage);
        self.render_scale.store(storage);
    }
}
