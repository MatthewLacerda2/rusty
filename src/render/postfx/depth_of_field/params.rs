//! What one view's depth of field draws with (#1445), from its volume: the
//! `DofParams` uniform `postfx/depth_of_field.wgsl` reads. Pure, so the circle of
//! confusion is tested without an adapter: [`DofParams::coc`] mirrors the shader's.

use crate::components::{DepthOfField, DofMode};

/// The Gaussian blur's radius at `gaussian_max_radius = 1`, as a fraction of the
/// view's height: 12 px at 1080p, Unity URP's Gaussian at its default radius.
pub(crate) const GAUSSIAN_RADIUS: f32 = 1.0 / 90.0;

/// The largest Bokeh circle of confusion, as a fraction of the view's height: 27 px
/// of radius at 1080p, 36 at 1440p, 54 at 4K. A wide-open telephoto clamps here,
/// and never past [`BOKEH_MAX_RINGS`] ring spacings (56 px), which only a view
/// taller than 4K reaches. The gather spreads its rings over each pixel's own reach,
/// at least three and never further apart than [`BOKEH_RING_SPACING`]: at the
/// largest circle 4 rings (80 taps) at 1080p, 5 at 1440p and 7 (224 taps) at 4K.
pub(crate) const MAX_BOKEH_RADIUS: f32 = 1.0 / 40.0;

/// The most full-resolution pixels between two rings of the Bokeh gather: the tent
/// smooths a gap this wide, where a wider one shows copies of a highlight.
pub(crate) const BOKEH_RING_SPACING: f32 = 8.0;

/// The most rings a Bokeh gather takes, which caps its circle at this many spacings.
pub(crate) const BOKEH_MAX_RINGS: f32 = 7.0;

/// The sensor height the Bokeh lens images onto, in metres: a 35 mm full frame's,
/// Unity's physical camera default.
pub(crate) const SENSOR_HEIGHT: f32 = 0.024;

/// CPU-side mirror of `DofParams` in `postfx/depth_of_field.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DofParams {
    /// mode (1 Gaussian, 2 Bokeh), the largest circle of confusion in full-resolution
    /// pixels, the working-resolution divisor, and Bokeh's tile size in working pixels
    /// (0 for Gaussian, which has no tiles)
    pub shape: [f32; 4],
    /// Gaussian: start and end distance (m), then unused. Bokeh: focus distance (m),
    /// the circle of confusion, in pixels, of a point at infinity, the most working
    /// pixels between two rings of the gather, then unused.
    pub focus: [f32; 4],
}

impl DofParams {
    /// The params for `dof` over a view `height` pixels tall, blurred at
    /// `1 / divisor` resolution; `None` when its mode is `Off`.
    pub(crate) fn new(dof: &DepthOfField, height: u32, divisor: u32) -> Option<Self> {
        let h = height as f32;
        let divisor = divisor.max(1) as f32;
        let (mode, max, focus) = match dof.mode {
            DofMode::Off => return None,
            DofMode::Gaussian => {
                let radius = dof.gaussian_max_radius * GAUSSIAN_RADIUS * h;
                let start = dof.gaussian_start.max(0.0);
                (1.0, radius, [start, dof.gaussian_end.max(start), 0.0, 0.0])
            }
            DofMode::Bokeh => {
                let infinity = bokeh_infinity(dof, h);
                let spacing = BOKEH_RING_SPACING / divisor;
                let focus = [dof.focus_distance, infinity, spacing, 0.0];
                (2.0, max_bokeh_radius(h), focus)
            }
        };
        // A tile spans the largest working CoC, so a 3×3 of tiles holds every reach.
        let tile = if mode > 1.5 {
            (max / divisor).ceil().max(1.0)
        } else {
            0.0
        };
        Some(Self {
            shape: [mode, max, divisor, tile],
            focus,
        })
    }

    /// The working-resolution divisor.
    pub(crate) fn divisor(&self) -> u32 {
        self.shape[2] as u32
    }

    /// Bokeh's tile size in working pixels; 0 for Gaussian.
    pub(crate) fn tile(&self) -> u32 {
        self.shape[3] as u32
    }

    /// The signed circle of confusion, in full-resolution pixels of radius, of a
    /// surface `depth` metres along its camera's view axis: negative nearer than the focus,
    /// positive beyond it, clamped to the largest. The shader's `coc_of`, for tests.
    #[cfg(test)]
    pub(crate) fn coc(&self, depth: f32) -> f32 {
        let max = self.shape[1];
        let [a, b, ..] = self.focus;
        if self.shape[0] < 1.5 {
            let ramp = ((depth - a) / (b - a).max(1e-4)).clamp(0.0, 1.0);
            return ramp * max;
        }
        (b * (1.0 - a / depth.max(1e-4))).clamp(-max, max)
    }
}

/// The largest Bokeh circle of confusion, in pixels, over a view `height` tall.
pub(crate) fn max_bokeh_radius(height: f32) -> f32 {
    (MAX_BOKEH_RADIUS * height).min(BOKEH_MAX_RINGS * BOKEH_RING_SPACING)
}

/// The circle of confusion's radius, in pixels of a view `height` tall, of a point
/// at infinity through `dof`'s lens: the thin-lens blur `f² / (N (F − f))` on the
/// sensor, scaled to the view. A point at depth `d` blurs by this times
/// `1 − F / d`.
fn bokeh_infinity(dof: &DepthOfField, height: f32) -> f32 {
    let f = dof.focal_length / 1000.0;
    let n = dof.aperture.max(DepthOfField::MIN_APERTURE);
    let past_lens = (dof.focus_distance - f).max(1e-3);
    let on_sensor = f * f / (n * past_lens);
    0.5 * on_sensor / SENSOR_HEIGHT * height
}

#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;
