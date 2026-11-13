//! crates/components/src/rendering/camera/target.rs — a camera's render-texture target
//! (#430, Unity's `Camera.targetTexture`) and the `"rt:"` naming every consumer
//! shares.

use serde::{Deserialize, Serialize};

/// A camera's render-texture target (#430, Unity's `Camera.targetTexture`): the
/// texture it draws into and what that costs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenderTarget {
    /// The texture's name; consumers reference it as `"rt:<name>"`.
    pub name: String,
    /// Resolution in pixels (each at least 1).
    pub width: u32,
    pub height: u32,
    /// Run the full post-FX chain (grading, bloom, FXAA, motion blur). Off, the
    /// camera is only tonemapped — cheaper, and right for a flat minimap.
    #[serde(default = "default_true")]
    pub post_fx: bool,
    /// Redraw every `n`th frame (1 = every frame); the texture holds its last
    /// picture in between — a security monitor at 1/4 rate is a quarter the cost.
    #[serde(default = "default_every")]
    pub update_every: u32,
}

impl RenderTarget {
    /// A target named `name` at `width` x `height`, post-FX on, every frame.
    pub fn new(name: &str, width: u32, height: u32) -> Self {
        Self {
            name: name.to_string(),
            width: width.max(1),
            height: height.max(1),
            post_fx: true,
            update_every: 1,
        }
    }

    /// The path a UI `Image` or a material map uses to show this texture.
    pub fn path(&self) -> String {
        format!("{RENDER_TEXTURE_PREFIX}{}", self.name)
    }
}

/// The prefix that marks a texture path as a render texture's name (#430) — one
/// convention for every consumer (`Image.texture`, material maps).
pub const RENDER_TEXTURE_PREFIX: &str = "rt:";

/// Whether `path` names a render texture rather than a file.
pub fn is_render_texture(path: &str) -> bool {
    path.starts_with(RENDER_TEXTURE_PREFIX)
}

fn default_true() -> bool {
    true
}

fn default_every() -> u32 {
    1
}
