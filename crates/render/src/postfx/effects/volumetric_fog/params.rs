use glam::{Mat4, Vec3, Vec4};

use super::{VolumetricPlan, HISTORY_WEIGHT};
use crate::postfx::effects::taa::jitter::halton;

/// Mirrors `Params` in `volumetric_fog.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct VolumetricParams {
    prev_view_proj: [f32; 16],
    prev_view_z: [f32; 4],
    prev_volumetric: [f32; 4],
    grid: [u32; 4],
    /// The sample point in each froxel; w this frame's weight (1: no history).
    jitter: [f32; 4],
    medium: [f32; 4],
    albedo: [f32; 4],
    probes: [[f32; 4]; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Built {
    pub view_proj: Mat4,
    pub view_z: Vec4,
    pub volumetric: [f32; 4],
}

impl Built {
    /// The history after the world moved by `by`: the projection and the
    /// view-depth plane (`n·world + w`) both move with it.
    pub(crate) fn shifted(self, by: Vec3) -> Self {
        let view_proj = crate::motion::shifted_view(self.view_proj, by);
        let n = self.view_z.truncate();
        let view_z = n.extend(self.view_z.w - n.dot(by));
        Self {
            view_proj,
            view_z,
            ..self
        }
    }
}

impl VolumetricParams {
    pub(crate) fn new(
        plan: &VolumetricPlan,
        (index, probes): (u32, [[f32; 4]; 2]),
        prev: Option<Built>,
    ) -> Self {
        let [w, h, d] = plan.grid;
        // From 1: `halton(0, b)` is 0, the eye itself.
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

    #[cfg(test)]
    pub(crate) fn weight(&self) -> f32 {
        self.jitter[3]
    }
}
