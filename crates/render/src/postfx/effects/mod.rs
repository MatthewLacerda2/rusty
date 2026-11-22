use crate::draw::stack::StackPass;
use crate::postfx::params::{effect_runs, Gate, PostSource};
use crate::{RenderView, Renderer};
use rusty_components::PostProcessingComponent;
use rusty_core::settings::GraphicsSettings;
use rusty_scene::Scene;

pub(crate) mod assemble;
mod layout;
mod passes;
mod targets;
pub(crate) mod textures;

// sorted: one module per effect (or family of effects); none reads another
pub(crate) mod bloom;
pub(crate) mod custom_effects;
pub(crate) mod depth_of_field;
pub(crate) mod exposure;
pub(crate) mod fxaa;
pub(crate) mod grading;
pub(crate) mod lens;
pub(crate) mod lens_flare;
pub(crate) mod motion_blur;
pub(crate) mod ssil;
pub(crate) mod ssr;
pub(crate) mod taa;
pub(crate) mod volumetric_fog;

pub(crate) use layout::{
    composite_layout, composite_slots, composite_slots_len, grade_layout, grade_slots,
    grade_slots_len, ldr_passes, scene_passes, Placed,
};
pub(crate) use passes::Passes;
pub(crate) use targets::Targets;
pub(crate) use textures::CompositeTexture;

use grading::{color_adjustments, color_lookup, lift_gamma_gain, tonemapping, white_balance};
use lens::{chromatic_aberration, film_grain, vignette};

pub(crate) const POST_EFFECTS: &[PostEffect] = &[
    // sorted: one row per effect; stages and orders place them, not this list
    bloom::EFFECT,
    chromatic_aberration::EFFECT,
    color_adjustments::EFFECT,
    color_lookup::EFFECT,
    custom_effects::EFFECT,
    depth_of_field::EFFECT,
    exposure::EFFECT,
    film_grain::EFFECT,
    fxaa::EFFECT,
    lens_flare::EFFECT,
    lift_gamma_gain::EFFECT,
    motion_blur::EFFECT,
    ssil::EFFECT,
    ssr::EFFECT,
    taa::EFFECT,
    tonemapping::EFFECT,
    vignette::EFFECT,
    volumetric_fog::EFFECT,
    white_balance::EFFECT,
];

pub(crate) struct PostEffect {
    pub name: &'static str,
    pub owner: Owner,
    pub gate: Gate,
    pub history: History,
    pub module: Option<&'static str>,
    pub composite: &'static [Composite],
    pub grade: &'static [Grade],
    pub passes: &'static [Pass],
}

/// If no effect reads history, redrawing with no sim step gives the same image, so
/// `Harness.Screenshot`'s settle draws no more.
#[derive(Clone, Copy)]
pub(crate) enum History {
    None(&'static str),
    Reads(fn(&HistoryCx) -> bool),
}

pub(crate) struct HistoryCx<'a> {
    pub effect: EffectCx<'a>,
    pub shared: &'a crate::draw::camera::StackShared,
    pub taa: bool,
}

pub(crate) fn read_history(cx: &HistoryCx) -> bool {
    (POST_EFFECTS.iter()).any(|e| matches!(e.history, History::Reads(runs) if runs(cx)))
}

/// Public as a test hook: the completeness gate's layers name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Volume,
    Camera,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Phase {
    Hdr,
    Display,
}

pub(crate) struct Composite {
    pub phase: Phase,
    pub order: u16,
    pub function: &'static str,
    pub slots: usize,
    pub textures: &'static [CompositeTexture],
    pub pack: fn(&EffectCx, &mut [[f32; 4]]),
}

pub(crate) struct Grade {
    pub order: u16,
    pub function: &'static str,
    pub slots: usize,
    pub pack: fn(&GradeCx, &mut [[f32; 4]]),
}

pub(crate) enum Pass {
    /// Over the HDR scene, before the composite.
    Scene { order: u16, run: ScenePass },
    Ldr {
        order: u16,
        prepare: fn(&mut super::PostFx, &wgpu::Device, &EffectCx) -> bool,
        run: LdrRun,
    },
}

pub(crate) type ScenePass =
    fn(&Renderer, &mut RenderView, &PassCx, &mut crate::gpu::commands::Commands);

pub(crate) enum LdrRun {
    InPlace(
        fn(&mut super::PostFx, Gpu, &mut wgpu::CommandEncoder, &super::PostFxContext, &EffectCx),
    ),
    MayEnd(
        fn(
            &mut super::PostFx,
            Gpu,
            &mut wgpu::CommandEncoder,
            &super::PostFxContext,
            &EffectCx,
            bool,
        ) -> bool,
    ),
    Finish(fn(&super::PostFx) -> &wgpu::RenderPipeline),
}

pub(crate) type Gpu<'a> = (&'a wgpu::Device, &'a wgpu::Queue);

#[derive(Clone, Copy)]
pub(crate) struct EffectCx<'a> {
    pub post: PostSource<'a>,
    pub graphics: GraphicsSettings,
    pub full: bool,
}

impl<'a> EffectCx<'a> {
    pub(crate) fn volume(&self) -> Option<&'a PostProcessingComponent> {
        self.post.volume.filter(|_| self.full)
    }

    pub(crate) fn runs(&self, gate: Gate, on: bool) -> bool {
        self.full && effect_runs(self.graphics, gate, on)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct GradeCx<'a> {
    pub volume: Option<&'a PostProcessingComponent>,
    pub lookup: Option<([[f32; 3]; 2], f32)>,
}

pub(crate) struct PassCx<'a> {
    pub scene: &'a Scene,
    pub stack: &'a StackPass<'a>,
    pub effect: EffectCx<'a>,
}

#[cfg(test)]
mod chain_size_tests;
#[doc(hidden)]
pub mod completeness;
#[cfg(test)]
mod pack_tests;
#[cfg(test)]
mod tests;
