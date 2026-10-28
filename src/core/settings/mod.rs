//! The engine's global settings resources: the quality preset, the player's
//! graphics options, the video settings and the game's decal budgets.

pub mod decals;
pub mod graphics_options;
pub mod quality;
pub mod video;

use self::graphics_options::GraphicsOptions;
use self::quality::QualityPreset;
use crate::core::storage::Storage;

/// The `Storage` namespace the graphics settings persist under: the quality tier
/// (key [`QUALITY_KEY`]) and the player's options
/// ([`OPTIONS_KEY`](graphics_options::OPTIONS_KEY)).
pub const GRAPHICS_NAMESPACE: &str = "graphics";
/// The quality tier's `Storage` key.
pub const QUALITY_KEY: &str = "quality";

/// What the renderer's post-FX chain is gated by besides the camera: the quality
/// tier and the player's options (#1438). One shared cell carries it from the
/// `Graphics` namespace (and the editor header) to the renderer each frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GraphicsSettings {
    pub quality: QualityPreset,
    pub options: GraphicsOptions,
}

impl From<QualityPreset> for GraphicsSettings {
    /// `quality` with every player option allowed.
    fn from(quality: QualityPreset) -> Self {
        Self {
            quality,
            options: GraphicsOptions::default(),
        }
    }
}

impl GraphicsSettings {
    /// The saved tier (the default tier when none) and the player's saved options
    /// over the project's `defaults`: what startup reads.
    pub fn load(storage: &Storage, defaults: GraphicsOptions) -> Self {
        let quality = storage
            .get(GRAPHICS_NAMESPACE, QUALITY_KEY)
            .and_then(|v| v.as_str().and_then(QualityPreset::parse))
            .unwrap_or_default();
        Self {
            quality,
            options: GraphicsOptions::load(storage, defaults),
        }
    }

    /// Write the tier and the player's options into `storage`; the Stop and quit
    /// flushes put them on disk.
    pub fn store(&self, storage: &mut Storage) {
        let quality = serde_json::Value::String(self.quality.name().to_string());
        storage.set(GRAPHICS_NAMESPACE, QUALITY_KEY, quality);
        self.options.store(storage);
    }
}
