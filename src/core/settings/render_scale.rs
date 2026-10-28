//! src/core/settings/render_scale.rs — `RenderScale`: the scene's render resolution
//! as a factor of the surface's (#1498).
//!
//! Unity URP's *Render Scale* (`UniversalRenderPipelineAsset.renderScale`): every
//! scene render target of the screen's camera stack (colour, depth, the SSAO prepass,
//! motion vectors, the post-FX targets) is sized `surface × scale`, and the finished,
//! post-processed image is upscaled to the surface with bilinear filtering before the
//! UI, which stays at native resolution. On a 2× Retina surface, 0.5 renders the
//! scene at the 1× pixel count. Persisted beside the quality tier, a player setting
//! like it. Render-only: never read by `FixedUpdate`.

use super::GRAPHICS_NAMESPACE;
use crate::core::storage::Storage;

/// The render scale's `Storage` key, in the graphics namespace.
pub const RENDER_SCALE_KEY: &str = "render_scale";

/// A render-scale factor, always inside [`RenderScale::MIN`]..=[`RenderScale::MAX`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderScale(f32);

// A `RenderScale` is never NaN (`new` maps it to 1), so equality is total.
impl Eq for RenderScale {}

impl Default for RenderScale {
    fn default() -> Self {
        Self::NATIVE
    }
}

impl RenderScale {
    /// The smallest factor, URP's.
    pub const MIN: f32 = 0.1;
    /// The largest factor (supersampling), URP's.
    pub const MAX: f32 = 2.0;
    /// The surface's own resolution: no scaling and no upscale pass.
    pub const NATIVE: Self = Self(1.0);

    /// `scale` clamped to the range; a NaN is the native scale.
    pub fn new(scale: f32) -> Self {
        match scale.is_nan() {
            true => Self::NATIVE,
            false => Self(scale.clamp(Self::MIN, Self::MAX)),
        }
    }

    /// The factor.
    pub fn get(self) -> f32 {
        self.0
    }

    /// `size` (a surface's width or height in pixels) at this scale, at least 1.
    pub fn apply(self, size: u32) -> u32 {
        ((size as f32 * self.0).round() as u32).max(1)
    }

    /// The saved scale, or native when none (or not a number).
    pub fn load(storage: &Storage) -> Self {
        storage
            .get(GRAPHICS_NAMESPACE, RENDER_SCALE_KEY)
            .and_then(|v| v.as_f64())
            .map_or(Self::NATIVE, |v| Self::new(v as f32))
    }

    /// Write the scale into `storage`.
    pub fn store(self, storage: &mut Storage) {
        let value = serde_json::Value::from(f64::from(self.0));
        storage.set(GRAPHICS_NAMESPACE, RENDER_SCALE_KEY, value);
    }
}

#[cfg(test)]
#[path = "render_scale_tests.rs"]
mod tests;
