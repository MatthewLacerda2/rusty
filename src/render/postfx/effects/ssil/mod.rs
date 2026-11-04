//! Screen-space indirect lighting (#1750), Godot 4's SSIL: one bounce of the frame's
//! realtime light off the surfaces on screen, added to the forward pass's indirect
//! diffuse. Like SSR it is in no post-chain stage: two of `render::draw::camera`'s
//! stages run it, on the base camera of a view whose volume turns it on.
//!
//! - **Before the forward pass** ([`run`]'s `record_ssil`, beside SSAO's passes): over
//!   the SSAO prepass's depth (SSIL runs that prepass itself when AO is off), a
//!   hemisphere gather at the quality's resolution reads **last frame's** light,
//!   reprojected through last frame's view-projection, then a depth-aware blur, a
//!   temporal accumulate and an upsample to full resolution. The forward pass reads
//!   the result at its group 3, in the opaque copy's slot (`t_opaque`, which that
//!   pass never reads: the stage's 16 sampled textures are all taken) while the
//!   camera uniform's `stack.z` says it runs; adds it to the ambient irradiance
//!   (`forward::lighting::indirect_bounce`) through the same `albedo × 0.96 ×
//!   (1 − metallic)` response and the same `ao` (SSAO's contact occlusion is
//!   visibility the half-resolution gather misses), so a metal takes none.
//! - **After the base camera's opaque pass** (the `ssil_source` stage, before SSR):
//!   its HDR colour, scene-linear before bloom, the grade, the tonemap, the UI, the
//!   translucents and any overlay camera (a viewmodel), times the **share** of it
//!   that is realtime light, is kept at the gather's resolution as next frame's
//!   source ([`targets`]).
//!
//! **Never twice over baked bounce.** The share is what the opaque pass wrote in its
//! alpha while SSIL runs (`forward::lighting::with_ssil_share`): the pixel's light less
//! its indirect term (lightmap, probes or flat ambient, and SSIL itself), its
//! environment reflection and, on a lightmapped surface, its emission, over the
//! whole. So the gather sees the light of `Realtime` and `Mixed` lights and of
//! unbaked emission: a lightmapped scene with only `Baked` lights gathers nothing
//! and brightens by nothing; a `Mixed` light's bounce overlaps the indirect light its
//! bake stored (as in Godot, which adds SSIL over every bake). Unlit surfaces and
//! the sky feed nothing. A transparent surface reads none (its group 3 is the no-AO
//! one, as for `t_ao`), and an overlay camera reads none.
//!
//! **History.** A cut (a scene load, Play or Stop, a camera switch: `motion`'s
//! `HistoryKey`), a resize, a quality change and the first frame it runs leave no
//! history: that frame gathers nothing and the next one starts over. Bake captures
//! and the shader preview, which keep no history of a view, never run it; a render
//! texture's camera keeps its own view and runs it there.
//!
//! **Cost.** Off, nothing: no target, no pass, and the forward pass reads no
//! bounce (`stack.z` is 0). On, per frame: the prepass when SSAO doesn't run
//! one, `samples` taps of five depths and one colour per gather texel, a 49-tap blur,
//! a 9-tap accumulate and the 4-tap upsample, plus the source's block average. Its
//! targets are five `Rgba16Float` textures at the gather's resolution and one at the
//! view's; no new binding. `Debug.Stats().gpu_passes.ssil` times every pass, and `ssil_samples`
//! counts the gather's taps.

mod pass;
mod run;
mod targets;

pub(crate) use pass::SsilPasses;
pub(crate) use targets::SsilTargets;

/// The forward group 3 SSIL built for this frame, when the view holds its state.
pub(crate) fn forward_group(post_fx: &crate::render::postfx::PostFx) -> Option<&wgpu::BindGroup> {
    post_fx.targets.get::<SsilTargets>()?.forward_group.as_ref()
}

use glam::{Mat4, Vec3};

use super::{Owner, PostEffect};
use crate::components::Ssil;
use crate::core::settings::graphics_options::GraphicsOption;
use crate::core::settings::quality::{SsilQuality, MAX_RESOLUTION_DIVISOR};
use crate::core::settings::GraphicsSettings;
use crate::render::draw::stack::StackPass;
use crate::render::motion::HistoryKey;
use crate::render::postfx::params::{effect_runs, Gate, PostSource};

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "ssil",
    owner: Owner::Volume,
    gate: Gate::option(GraphicsOption::Ssil),
    module: None,
    composite: &[],
    grade: &[],
    pass: None,
};

/// What a view's SSIL does this frame, when it runs: the volume's look within its
/// range and its quality's cost.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SsilPlan {
    pub intensity: f32,
    pub radius: f32,
    pub quality: SsilQuality,
}

impl SsilPlan {
    /// The plan of a view whose base camera's settings are `post`, under `graphics`:
    /// its volume with SSIL on at an intensity above zero and the player's `ssil`
    /// option allowing it. `full` is the stack's `full_post_fx`. `None`, nothing
    /// runs, otherwise. Every value is clamped to its range here (a NaN to the
    /// default), so a file that skipped the ops never reaches the GPU out of it.
    pub(crate) fn for_view(
        post: PostSource,
        graphics: GraphicsSettings,
        full: bool,
    ) -> Option<Self> {
        let s = post.volume?.ssil;
        let on = s.active && s.intensity > 0.0;
        if !(full && effect_runs(graphics, EFFECT.gate, on)) {
            return None;
        }
        let custom = SsilQuality {
            samples: s.samples,
            resolution_divisor: s.resolution_divisor,
        };
        let picked = s.quality.pick(&graphics.quality.ssil, custom);
        let defaults = Ssil::default();
        let radius = if s.radius.is_nan() {
            defaults.radius
        } else {
            s.radius
        };
        Some(Self {
            intensity: s.intensity.min(Ssil::MAX_INTENSITY),
            radius: radius.clamp(Ssil::MIN_RADIUS, Ssil::MAX_RADIUS),
            quality: SsilQuality {
                samples: picked.samples.clamp(1, Ssil::MAX_SAMPLES),
                resolution_divisor: picked.resolution_divisor.clamp(1, MAX_RESOLUTION_DIVISOR),
            },
        })
    }
}

/// The SSIL of a view drawing `stack` this frame ([`SsilPlan::for_view`]) under
/// `graphics`, with its history `key`, when it runs, and only on a view that
/// `keeps` a history; else drop its state, so off holds nothing.
pub(crate) fn begin(
    post_fx: &mut crate::render::postfx::PostFx,
    (stack, graphics, key): (&StackPass, GraphicsSettings, HistoryKey),
    keeps: bool,
) -> Option<SsilView> {
    let plan = SsilPlan::for_view(stack.post, graphics, stack.full_post_fx);
    let plan = plan.filter(|_| keeps);
    if plan.is_none() {
        post_fx.targets.remove::<SsilTargets>();
    }
    plan.map(|plan| (plan, key))
}

/// One base camera's SSIL inputs: the plan, where the camera looks, and what its
/// history is valid for.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SsilFrame {
    pub plan: SsilPlan,
    pub view_proj: Mat4,
    pub eye: Vec3,
    pub forward: Vec3,
    pub key: HistoryKey,
}

/// A view's SSIL this frame and what its history is valid for (a cut changes it).
pub(crate) type SsilView = (SsilPlan, HistoryKey);

impl SsilFrame {
    /// The frame of `camera`, drawn with `view_proj`, when `ssil` runs on it (the
    /// stack's base camera alone).
    pub(crate) fn for_camera(
        ssil: Option<SsilView>,
        view_proj: Mat4,
        camera: &crate::scene::Camera,
    ) -> Option<Self> {
        ssil.map(|(plan, key)| Self {
            plan,
            view_proj,
            eye: camera.position,
            forward: camera.forward(),
            key,
        })
    }
}

/// CPU mirror of `SsilParams` in `postfx/ssil.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SsilUniform {
    inv_view_proj: [f32; 16],
    view_proj: [f32; 16],
    prev_view_proj: [f32; 16],
    /// w: the frame's index, which turns the noise.
    camera_pos: [f32; 4],
    /// w: 1 with a history, 0 on a cut.
    camera_fwd: [f32; 4],
    prev_camera_pos: [f32; 4],
    /// radius, intensity, sample count, full-res pixels per texel
    params: [f32; 4],
}

/// Last frame's camera, as the source and result were drawn with it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Previous {
    pub view_proj: Mat4,
    pub eye: Vec3,
}

impl SsilFrame {
    /// The uniform every stage reads: `prev` is last frame's camera, `None` on a cut
    /// (the gather then adds nothing); `index` turns the noise.
    pub(crate) fn uniform(&self, prev: Option<Previous>, index: u32) -> SsilUniform {
        let p = self.plan;
        let had = prev.is_some();
        let prev = prev.unwrap_or(Previous {
            view_proj: self.view_proj,
            eye: self.eye,
        });
        SsilUniform {
            inv_view_proj: self.view_proj.inverse().to_cols_array(),
            view_proj: self.view_proj.to_cols_array(),
            prev_view_proj: prev.view_proj.to_cols_array(),
            camera_pos: self.eye.extend(index as f32).to_array(),
            camera_fwd: self
                .forward
                .normalize()
                .extend(f32::from(u8::from(had)))
                .to_array(),
            prev_camera_pos: prev.eye.extend(0.0).to_array(),
            params: [
                p.radius,
                p.intensity,
                p.quality.samples as f32,
                p.quality.resolution_divisor as f32,
            ],
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod gpu;
