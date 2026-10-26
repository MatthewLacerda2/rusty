//! The engine's global settings resources: the quality preset, the player's
//! graphics options and the video settings.

pub mod graphics_options;
pub mod quality;
pub mod video;

use self::graphics_options::GraphicsOptions;
use self::quality::QualityPreset;

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
