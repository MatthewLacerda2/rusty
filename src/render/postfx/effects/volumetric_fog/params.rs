//! The froxel passes' uniform (#1703), `Params` in `volumetric_fog.wgsl`.

use glam::{Mat4, Vec4};

use super::{VolumetricPlan, HISTORY_WEIGHT};
use crate::render::postfx::effects::taa::jitter::halton;

/// CPU mirror of `Params` in `volumetric_fog.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct VolumetricParams {
    prev_view_proj: [f32; 16],
    prev_view_z: [f32; 4],
    prev_volumetric: [f32; 4],
    /// Froxels across, up and deep; w the frame's index.
    grid: [u32; 4],
    /// The sample point in each froxel; w this frame's weight (1: no history).
    jitter: [f32; 4],
    medium: [f32; 4],
    albedo: [f32; 4],
    /// The probe grid's box (`probes::AirProbes`): its corner, w 1 when the scene
    /// has probes; its size.
    probes: [[f32; 4]; 2],
}

/// The grid a view's history was built in: its camera's view-projection, view
/// plane and `volumetric` slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Built {
    pub view_proj: Mat4,
    pub view_z: Vec4,
    pub volumetric: [f32; 4],
}

impl VolumetricParams {
    /// This frame's uniform for `plan`, frame `index`, reprojecting from `prev`
    /// (`None`: no history).
    pub(crate) fn new(
        plan: &VolumetricPlan,
        (index, probes): (u32, [[f32; 4]; 2]),
        prev: Option<Built>,
    ) -> Self {
        let [w, h, d] = plan.grid;
        // From 1, as `taa::jitter::pixels`: `halton(0, b)` is 0, which would put a
        // frame's first slice sample at the eye itself.
        let i = index + 1;
        let jitter = [halton(i, 2), halton(i, 3), halton(i, 5)];
        let weight = if prev.is_some() {
            1.0 - HISTORY_WEIGHT
        } else {
            1.0
        };
        let prev = prev.unwrap_or(Built {
            view_proj: Mat4::IDENTITY,
            view_z: Vec4::ZERO,
            volumetric: [0.0, 1.0, 1.0, 1.0],
        });
        Self {
            prev_view_proj: prev.view_proj.to_cols_array(),
            prev_view_z: prev.view_z.to_array(),
            prev_volumetric: prev.volumetric,
            grid: [w, h, d, index],
            jitter: [jitter[0], jitter[1], jitter[2], weight],
            medium: [
                plan.extinction,
                plan.base_height,
                plan.falloff,
                plan.anisotropy,
            ],
            albedo: [plan.albedo[0], plan.albedo[1], plan.albedo[2], plan.ambient],
            probes,
        }
    }

    /// This frame's weight against the history: 1 when it has none.
    #[cfg(test)]
    pub(crate) fn weight(&self) -> f32 {
        self.jitter[3]
    }
}
