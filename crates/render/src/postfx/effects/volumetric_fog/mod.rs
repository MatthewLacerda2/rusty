//! Froxel scattering (Hillaire 2015), as UE's *Volumetric Fog* and HDRP's *Fog*.
//! Runs from `render::draw::camera` on a view's base camera only (an overlay camera
//! reads none), outside the post chain. Light the fog dimmed is not added again by
//! SSR or SSIL: both read `1 - (1 - fog) * transmittance`.

mod params;
mod pass;
mod probes;
mod run;
mod targets;
mod volume;

pub(crate) use params::VolumetricParams;
pub(crate) use pass::VolumetricFogPasses;
pub(crate) use run::for_camera;
pub(crate) use targets::VolumetricTargets;
pub(crate) use volume::SharedVolume;

use super::{History, Owner, PostEffect};
use crate::draw::stack::StackPass;
use crate::motion::HistoryKey;
use crate::postfx::params::{effect_runs, Gate, PostSource};
use rusty_components::VolumetricFog;
use rusty_core::settings::graphics_options::GraphicsOption;
use rusty_core::settings::quality::VolumetricFogQuality;
use rusty_core::settings::GraphicsSettings;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "volumetric_fog",
    owner: Owner::Volume,
    gate: Gate::option(GraphicsOption::VolumetricFog),
    history: History::Reads(|cx| cx.shared.volumetric.is_some()),
    module: None,
    composite: &[],
    grade: &[],
    passes: &[],
};

/// Metres of view depth where the exponential slices start.
pub(crate) const VOLUME_NEAR: f32 = 0.5;

/// The share of last frame's froxel each frame keeps.
pub(crate) const HISTORY_WEIGHT: f32 = 0.85;

/// `HISTORY_WEIGHT ^ GHOST_FRAMES < 0.05`.
#[cfg(test)]
pub(crate) const GHOST_FRAMES: u32 = 19;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct VolumetricPlan {
    /// Per metre at the base height.
    pub extinction: f32,
    pub albedo: [f32; 3],
    pub anisotropy: f32,
    pub base_height: f32,
    /// Per metre above the base height.
    pub falloff: f32,
    pub depth_extent: f32,
    pub ambient: f32,
    pub grid: [u32; 3],
}

impl VolumetricPlan {
    /// Values are clamped here (a NaN to the default): a file may skip the ops.
    pub(crate) fn for_view(
        post: PostSource,
        graphics: GraphicsSettings,
        full: bool,
    ) -> Option<Self> {
        let s = post.volume?.volumetric_fog;
        if !(full && effect_runs(graphics, EFFECT.gate, s.active)) {
            return None;
        }
        let d = VolumetricFog::default();
        let or = |v: f32, fallback: f32| if v.is_finite() { v } else { fallback };
        let mfp = or(s.mean_free_path, d.mean_free_path);
        let base = or(s.base_height, d.base_height);
        let top = or(s.maximum_height, d.maximum_height);
        let custom = VolumetricFogQuality {
            resolution: s.resolution,
            slices: s.slices,
        };
        let picked = s.quality.pick(&graphics.quality.volumetric_fog, custom);
        Some(Self {
            extinction: 1.0
                / mfp.clamp(
                    VolumetricFog::MIN_MEAN_FREE_PATH,
                    VolumetricFog::MAX_MEAN_FREE_PATH,
                ),
            albedo: s.albedo.map(|c| or(c, 1.0).clamp(0.0, 1.0)),
            anisotropy: or(s.anisotropy, 0.0).clamp(
                -VolumetricFog::MAX_ANISOTROPY,
                VolumetricFog::MAX_ANISOTROPY,
            ),
            base_height: base.clamp(-VolumetricFog::MAX_HEIGHT, VolumetricFog::MAX_HEIGHT),
            falloff: 1.0 / (top - base).max(0.01),
            depth_extent: or(s.depth_extent, d.depth_extent).clamp(
                VolumetricFog::MIN_DEPTH_EXTENT,
                VolumetricFog::MAX_DEPTH_EXTENT,
            ),
            ambient: or(s.global_light_probe_dimmer, 1.0).clamp(0.0, 1.0),
            grid: grid(picked),
        })
    }

    pub(crate) fn camera_slot(plan: Option<&Self>) -> [f32; 4] {
        plan.map_or([0.0; 4], |p| {
            [1.0, p.grid[2] as f32, p.depth_extent, VOLUME_NEAR]
        })
    }

    pub(crate) fn froxels(&self) -> u64 {
        self.grid.iter().map(|&n| u64::from(n)).product()
    }
}

pub(crate) fn grid(q: VolumetricFogQuality) -> [u32; 3] {
    let across = q
        .resolution
        .clamp(VolumetricFog::MIN_RESOLUTION, VolumetricFog::MAX_RESOLUTION);
    let up = (across * 9).div_ceil(16);
    [
        across,
        up,
        q.slices
            .clamp(VolumetricFog::MIN_SLICES, VolumetricFog::MAX_SLICES),
    ]
}

pub(crate) type VolumetricView = (VolumetricPlan, HistoryKey);

pub(crate) fn begin(
    post_fx: &mut crate::postfx::PostFx,
    (stack, graphics, key): (&StackPass, GraphicsSettings, HistoryKey),
    keeps: bool,
) -> Option<VolumetricView> {
    let plan = VolumetricPlan::for_view(stack.post, graphics, stack.full_post_fx);
    let plan = plan.filter(|_| keeps);
    if plan.is_none() {
        post_fx.targets.remove::<VolumetricTargets>();
    }
    plan.map(|plan| (plan, key))
}

#[cfg(test)]
mod gpu;
#[cfg(test)]
mod tests;
