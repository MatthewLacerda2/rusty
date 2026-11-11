//! Volumetric fog (#1703): Frostbite's froxel scattering (Hillaire 2015), as UE's
//! *Volumetric Fog*, HDRP's *Fog* with volumetrics and Godot 4 run it. Like SSIL it
//! is in no post-chain stage: `render::draw::camera`'s `volumetric_fog` stage runs it
//! on the base camera of a view whose volume turns it on, after the camera's lights
//! are binned and before anything draws.
//!
//! - **Scatter** (`volumetric_fog.wgsl`'s `cs_scatter`): over a froxel grid (the
//!   quality's `resolution` across, `9/16` of it up, `slices` deep, exponential from
//!   [`VOLUME_NEAR`] to the volume's `depth_extent`), the medium at a jittered point
//!   in each froxel (`1 / mean_free_path` at `base_height`, falling to `1/e` at
//!   `maximum_height`), lit by the ambient times `global_light_probe_dimmer` (the
//!   light probes' L0 on a coarse grid around the camera where the scene has
//!   probes, [`probes::AirProbes`]; else the scene ambient), the live
//!   suns (the first through its cascades), the clustered point and spot lights
//!   through their shadow-atlas tiles, and the `Baked` lamps unshadowed
//!   (`clusters::BakedLamps`: they are in no frame light list, and a lightmapped
//!   level would get no halo without them), each through Henyey-Greenstein's phase
//!   at `anisotropy`; then blended with last frame's froxel reprojected.
//! - **Integrate** (`cs_integrate`): front to back per column into the renderer's
//!   one integrated volume ([`SharedVolume`]), which group 0 binds (17, 18): the
//!   forward and transparent surfaces (decals fold into them), the sky and the
//!   particles read it through `fog::volumetric` (the skies through `sky::air`), so
//!   they agree. A surface's colour becomes `colour × transmittance + in-scatter`
//!   (an Additive surface is only dimmed, a Multiply one's tint fades toward white:
//!   `forward::transparent::blended_air`), after the distance and height fog
//!   (`scene::settings::fog`), which still draws.
//!
//! **One fog value downstream.** SSR's record and SSIL's realtime share read the
//! fog a surface got as one factor, `1 − (1 − fog) × transmittance`
//! (`forward::transparent::surface_fog`), so neither adds light the fog dimmed.
//! Exposure and bloom see the fogged image as they see fog today.
//!
//! **Temporal.** Each frame samples the froxels at another point of a Halton
//! sequence and keeps [`HISTORY_WEIGHT`] of last frame's froxel at the same world
//! point, reprojected through last frame's grid; a point last frame's grid didn't
//! cover, or a NaN, takes this frame's alone. A light that moves leaves under 5 %
//! of its old glow after [`GHOST_FRAMES`] frames. A cut (`motion`'s `HistoryKey`),
//! a resize, a change of grid and the first frame keep no history. With TAA on the
//! froxels use the jittered camera (the grid is far coarser than a pixel) and TAA
//! then smooths the surfaces as it does any lighting.
//!
//! **Which cameras.** The base camera of a view only. An overlay camera (a
//! viewmodel) reads none: its froxels would need its own projection, and at arm's
//! length the air in front of a gun is no fog. Bake captures and the shader
//! preview, which keep no history of a view, never run it; a render texture's
//! camera keeps its own view and runs it there when its `post_fx` is on.
//!
//! **Cost.** Off, no target, no pass and no texture read: the camera uniform's
//! `volumetric.x` is 0, a uniform branch in every reader, and the renderer's
//! integrated volume falls back to a one-texel stand-in once no camera has run it for
//! [`volume::IDLE_RENDERS`] renders. On: per froxel one jittered point, its
//! cluster's lights (≤ 32 live, ≤ 32 `Baked`, each with up to nine shadow taps) and
//! one history tap; per column a loop over the slices. Memory: two `Rgba16Float`
//! froxel grids per view (this frame and its history) and the integrated one per
//! renderer, 8 bytes a froxel each: 160×90×64 is 921 600 froxels, 7.4 MB a grid.
//! The `Baked` list is rebuilt only when a light, pose or activity changes, and
//! each camera bins it like its live lights (137 lamps bin in the same pass as
//! the frame's live ones). An M4 is expected to take 0.5–1 ms at 160×90×64
//! (inferred, a target, not a measurement). `Debug.Stats().gpu_passes.volumetric_fog`
//! times both passes and `volumetric_froxels` counts the froxels lit.

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
use crate::components::VolumetricFog;
use crate::core::settings::graphics_options::GraphicsOption;
use crate::core::settings::quality::VolumetricFogQuality;
use crate::core::settings::GraphicsSettings;
use crate::render::draw::stack::StackPass;
use crate::render::motion::HistoryKey;
use crate::render::postfx::params::{effect_runs, Gate, PostSource};

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

/// Where the exponential slices start, in metres of view depth: the first slice
/// runs from the eye to just past it.
pub(crate) const VOLUME_NEAR: f32 = 0.5;

/// The share of last frame's froxel each frame keeps.
pub(crate) const HISTORY_WEIGHT: f32 = 0.85;

/// Frames after which a light that moved keeps under 5 % of its old glow:
/// `HISTORY_WEIGHT ^ GHOST_FRAMES < 0.05`, which a test holds.
#[cfg(test)]
pub(crate) const GHOST_FRAMES: u32 = 19;

/// What a view's volumetric fog does this frame, when it runs: its medium within
/// range and its grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct VolumetricPlan {
    /// Extinction per metre at the base height: `1 / mean_free_path`.
    pub extinction: f32,
    pub albedo: [f32; 3],
    pub anisotropy: f32,
    pub base_height: f32,
    /// Per metre above the base height, the density's exponential falloff.
    pub falloff: f32,
    pub depth_extent: f32,
    pub ambient: f32,
    /// Froxels across, up and deep.
    pub grid: [u32; 3],
}

impl VolumetricPlan {
    /// The plan of a view whose base camera's settings are `post`, under
    /// `graphics`: its volume with volumetric fog on and the player's option
    /// allowing it. `full` is the stack's `full_post_fx`. Every value is brought
    /// into its range here (a NaN to the default), so a file that skipped the ops
    /// never reaches the GPU out of it.
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

    /// The camera uniform's `volumetric`: on, the slices, the far and near ends.
    pub(crate) fn camera_slot(plan: Option<&Self>) -> [f32; 4] {
        plan.map_or([0.0; 4], |p| {
            [1.0, p.grid[2] as f32, p.depth_extent, VOLUME_NEAR]
        })
    }

    /// Froxels in the grid.
    pub(crate) fn froxels(&self) -> u64 {
        self.grid.iter().map(|&n| u64::from(n)).product()
    }
}

/// The grid a quality value asks for, each count brought into its range: across,
/// up (`9/16` of across, rounded) and deep.
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

/// A view's volumetric fog this frame and what its history is valid for.
pub(crate) type VolumetricView = (VolumetricPlan, HistoryKey);

/// The volumetric fog of a view drawing `stack` this frame under `graphics`, with
/// its history `key`, when it runs, and only on a view that `keeps` a history;
/// else drop its state, so off holds nothing.
pub(crate) fn begin(
    post_fx: &mut crate::render::postfx::PostFx,
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
