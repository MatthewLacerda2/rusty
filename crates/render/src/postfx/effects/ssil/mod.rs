//! Godot 4's SSIL: one bounce of realtime light off on-screen surfaces. The source
//! keeps only the realtime share (`forward::lighting::ssil_realtime`), so baked
//! light is never bounced twice. The 49-tap blur is where to look if over budget.

mod pass;
mod run;
mod targets;

pub(crate) use pass::SsilPasses;
pub(crate) use targets::SsilTargets;

pub(crate) fn forward_group(post_fx: &crate::postfx::PostFx) -> Option<&wgpu::BindGroup> {
    post_fx.targets.get::<SsilTargets>()?.forward_group.as_ref()
}

use glam::{Mat4, Vec3};

use super::{History, Owner, PostEffect};
use crate::draw::stack::StackPass;
use crate::motion::HistoryKey;
use crate::postfx::params::{effect_runs, Gate, PostSource};
use rusty_components::Ssil;
use rusty_core::settings::graphics_options::GraphicsOption;
use rusty_core::settings::quality::{SsilQuality, MAX_RESOLUTION_DIVISOR};
use rusty_core::settings::GraphicsSettings;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "ssil",
    owner: Owner::Volume,
    gate: Gate::option(GraphicsOption::Ssil),
    history: History::Reads(|cx| cx.shared.ssil.is_some()),
    module: None,
    composite: &[],
    grade: &[],
    passes: &[],
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SsilPlan {
    pub intensity: f32,
    pub radius: f32,
    pub quality: SsilQuality,
}

impl SsilPlan {
    /// Values are clamped here (a NaN to the default): a file may skip the ops.
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

pub(crate) fn begin(
    post_fx: &mut crate::postfx::PostFx,
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

#[derive(Clone, Copy, Debug)]
pub(crate) struct SsilFrame {
    pub plan: SsilPlan,
    pub view_proj: Mat4,
    pub eye: Vec3,
    pub forward: Vec3,
    pub key: HistoryKey,
}

pub(crate) type SsilView = (SsilPlan, HistoryKey);

impl SsilFrame {
    pub(crate) fn for_camera(
        ssil: Option<SsilView>,
        view_proj: Mat4,
        camera: &rusty_scene::Camera,
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

/// Mirrors `SsilParams` in `postfx/ssil.wgsl`.
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Previous {
    pub view_proj: Mat4,
    pub eye: Vec3,
}

impl SsilFrame {
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
