//! Unity's `Camera.targetTexture`; consumers reference the texture as `"rt:<name>"`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenderTarget {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Off, the camera is only tonemapped: cheaper, right for a flat minimap.
    #[serde(default = "default_true")]
    pub post_fx: bool,
    /// Redraw every `n`th frame (1 = every frame); the texture holds its last picture between.
    #[serde(default = "default_every")]
    pub update_every: u32,
}

impl RenderTarget {
    pub fn new(name: &str, width: u32, height: u32) -> Self {
        Self {
            name: name.to_string(),
            width: width.max(1),
            height: height.max(1),
            post_fx: true,
            update_every: 1,
        }
    }

    pub fn path(&self) -> String {
        format!("{RENDER_TEXTURE_PREFIX}{}", self.name)
    }
}

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
