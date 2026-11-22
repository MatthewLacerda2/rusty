use rusty_components::{DepthOfField, DofMode};
use rusty_core::settings::quality::MAX_RESOLUTION_DIVISOR;

pub(crate) const GAUSSIAN_RADIUS: f32 = 1.0 / 90.0;

pub(crate) const MAX_BOKEH_RADIUS: f32 = 1.0 / 40.0;

pub(crate) const BOKEH_RING_SPACING: f32 = 8.0;

pub(crate) const BOKEH_MAX_RINGS: f32 = 7.0;

/// The sensor height the Bokeh lens images onto, in metres: a 35 mm full frame's,
/// Unity's physical camera default.
pub(crate) const SENSOR_HEIGHT: f32 = 0.024;

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DofParams {
    pub shape: [f32; 4],
    /// Gaussian: start and end distance (m), then unused. Bokeh: focus distance (m),
    /// the circle of confusion, in pixels, of a point at infinity, the most working
    /// pixels between two rings of the gather, then unused.
    pub focus: [f32; 4],
}

impl DofParams {
    pub(crate) fn new(dof: &DepthOfField, height: u32, divisor: u32) -> Option<Self> {
        let h = height as f32;
        let divisor = divisor.clamp(1, MAX_RESOLUTION_DIVISOR) as f32;
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

    pub(crate) fn divisor(&self) -> u32 {
        self.shape[2] as u32
    }

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

pub(crate) fn max_bokeh_radius(height: f32) -> f32 {
    (MAX_BOKEH_RADIUS * height).min(BOKEH_MAX_RINGS * BOKEH_RING_SPACING)
}

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
