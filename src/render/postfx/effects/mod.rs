//! src/render/postfx/effects/ — the post effects, one module per effect (#1467; the
//! grading and lens effects in their folders), listed once in [`POST_EFFECTS`].
//!
//! An effect is made of up to three parts, each placed by a **stage** and an
//! **order** it declares, never by its row's position:
//!
//! - **composite functions** ([`Composite`]): WGSL functions of its module
//!   (`engine/shaders/postfx/effects/<effect>.wgsl`) the composite calls on the HDR
//!   colour before the grade ([`Phase::Hdr`]) or on the display colour after it
//!   ([`Phase::Display`]). The composite is assembled from them ([`assemble`]). A
//!   function may read textures of its effect's own ([`CompositeTexture`]), which
//!   the registry binds ([`textures`], #1473);
//! - **grade functions** ([`Grade`]): functions the grading bake runs on each texel of
//!   the view's LUT, in order;
//! - a **pass** ([`Pass`]): its own GPU work, on the HDR scene before the composite
//!   ([`Pass::Scene`]: depth of field, motion blur, the exposure meter, bloom's
//!   bright pass, the lens flare) or on the LDR image after it ([`Pass::Ldr`]: the authored effects,
//!   FXAA).
//!
//! **Params without a shared struct.** Each function packs its own vec4 slots; the
//! composite's slots and the bake's are each one array, laid out from this registry
//! ([`composite_layout`], [`grade_layout`]), so a new effect moves no other's offsets
//! and grows no shared struct.

use crate::components::PostProcessingComponent;
use crate::core::settings::GraphicsSettings;
use crate::render::draw::stack::StackPass;
use crate::render::postfx::params::{effect_runs, Gate, PostSource};
use crate::render::{RenderView, Renderer};
use crate::scene::Scene;

pub(crate) mod assemble;
mod layout;
mod passes;
mod targets;
pub(crate) mod textures;

// sorted: one module per effect (or family of effects); none reads another
pub(crate) mod bloom;
pub(crate) mod custom_effects;
pub(crate) mod depth_of_field;
#[cfg(test)]
pub(crate) mod dummy;
pub(crate) mod exposure;
pub(crate) mod fxaa;
pub(crate) mod grading;
pub(crate) mod lens;
pub(crate) mod lens_flare;
pub(crate) mod motion_blur;
pub(crate) mod ssr;

pub(crate) use layout::{
    composite_layout, composite_slots, composite_slots_len, grade_layout, grade_slots,
    grade_slots_len, ldr_passes, scene_passes, Placed,
};
pub(crate) use passes::Passes;
pub(crate) use targets::Targets;
pub(crate) use textures::CompositeTexture;

use grading::{color_adjustments, color_lookup, lift_gamma_gain, tonemapping, white_balance};
use lens::{chromatic_aberration, film_grain, vignette};

/// Every post effect, by name.
pub(crate) const POST_EFFECTS: &[PostEffect] = &[
    // sorted: one row per effect; stages and orders place them, not this list
    bloom::EFFECT,
    chromatic_aberration::EFFECT,
    color_adjustments::EFFECT,
    color_lookup::EFFECT,
    custom_effects::EFFECT,
    depth_of_field::EFFECT,
    #[cfg(test)]
    dummy::EFFECT,
    exposure::EFFECT,
    film_grain::EFFECT,
    fxaa::EFFECT,
    lens_flare::EFFECT,
    lift_gamma_gain::EFFECT,
    motion_blur::EFFECT,
    ssr::EFFECT,
    tonemapping::EFFECT,
    vignette::EFFECT,
    white_balance::EFFECT,
];

/// One post effect's render parts.
pub(crate) struct PostEffect {
    /// As every layer names it: the volume's field, or the camera effect's name.
    pub name: &'static str,
    /// Where its settings live.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the completeness test reads it, to check each layer"
        )
    )]
    pub owner: Owner,
    /// The player option that gates it, when it is player-facing (#1438), and the
    /// quality tiers it runs on: its own rule, asked through [`EffectCx::runs`].
    pub gate: Gate,
    /// Its WGSL module, under `engine/shaders`, when it has composite or grade
    /// functions. Its import path is the file's path (`postfx::effects::vignette`).
    pub module: Option<&'static str>,
    pub composite: &'static [Composite],
    pub grade: &'static [Grade],
    pub pass: Option<Pass>,
}

/// Where an effect's settings live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Owner {
    /// A field of the post-processing volume.
    Volume,
    /// The camera's own component (motion blur and FXAA, Unity's camera properties).
    Camera,
}

/// Which side of the grade a composite function runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Phase {
    /// On the scene-linear HDR colour, before the grading LUT.
    Hdr,
    /// On the graded display colour, after it.
    Display,
}

/// A WGSL function the composite calls: `fn <function>(c: vec3<f32>, f: Frag,
/// <one texture_2d<f32> per texture>, <one vec4 per slot>) -> vec3<f32>` in the
/// effect's module.
pub(crate) struct Composite {
    pub phase: Phase,
    /// Within the phase, lower first.
    pub order: u16,
    pub function: &'static str,
    /// How many vec4 params it reads.
    pub slots: usize,
    /// The textures of its own it reads, passed to it in this order.
    pub textures: &'static [CompositeTexture],
    /// Its params for this frame, written into its `slots` slots. An effect that
    /// doesn't run packs what its function skips on, so it draws as if absent.
    pub pack: fn(&EffectCx, &mut [[f32; 4]]),
}

/// A WGSL function the grading bake calls on each texel: `fn <function>(c: vec3<f32>,
/// <one vec4 per slot>) -> vec3<f32>`.
pub(crate) struct Grade {
    /// Lower first.
    pub order: u16,
    pub function: &'static str,
    pub slots: usize,
    /// Its params for this bake; the grade's neutral values without a volume.
    pub pack: fn(&GradeCx, &mut [[f32; 4]]),
}

/// An effect's own GPU work.
pub(crate) enum Pass {
    /// Over the HDR scene, before the composite, in `order`.
    Scene { order: u16, run: ScenePass },
    /// Over the composite's LDR image, after it, in `order`.
    Ldr {
        order: u16,
        /// Get ready for this frame (compile, upload) and say whether it runs.
        prepare: fn(&mut super::PostFx, &wgpu::Device, &EffectCx) -> bool,
        run: LdrRun,
    },
}

/// A scene pass: the renderer's shared pipelines, the view, and the frame.
pub(crate) type ScenePass = fn(&Renderer, &mut RenderView, &PassCx);

/// How an LDR pass runs.
pub(crate) enum LdrRun {
    /// Reads [`super::PostFx::ldr`] and leaves its result there.
    InPlace(
        fn(&mut super::PostFx, Gpu, &mut wgpu::CommandEncoder, &super::PostFxContext, &EffectCx),
    ),
    /// The finishing pass: this pipeline, `ldr` in, the chain's output out. At most
    /// one runs, last; without one the chain copies `ldr` out.
    Finish(fn(&super::PostFx) -> &wgpu::RenderPipeline),
}

/// The device and queue, together.
pub(crate) type Gpu<'a> = (&'a wgpu::Device, &'a wgpu::Queue);

/// What an effect packs from: one view's camera and volume, the player's options
/// and the tier, and whether the view runs post-FX at all (a render texture's
/// `post_fx = false` brings the image to the display only).
#[derive(Clone, Copy)]
pub(crate) struct EffectCx<'a> {
    pub post: PostSource<'a>,
    pub graphics: GraphicsSettings,
    pub full: bool,
}

impl<'a> EffectCx<'a> {
    /// The volume, when the view runs post-FX and its camera's volume is active.
    pub(crate) fn volume(&self) -> Option<&'a PostProcessingComponent> {
        self.post.volume.filter(|_| self.full)
    }

    /// Whether an effect the settings have `on` runs, gated by its `gate`.
    pub(crate) fn runs(&self, gate: Gate, on: bool) -> bool {
        self.full && effect_runs(self.graphics, gate, on)
    }
}

/// What a grade function packs from.
#[derive(Clone, Copy)]
pub(crate) struct GradeCx<'a> {
    /// The view's volume (`None`: no grade, no tonemap).
    pub volume: Option<&'a PostProcessingComponent>,
    /// The Color Lookup's uploaded LUT, when one applies: its input domain and the
    /// contribution.
    pub lookup: Option<([[f32; 3]; 2], f32)>,
}

/// What a scene pass runs with.
pub(crate) struct PassCx<'a> {
    pub scene: &'a Scene,
    pub stack: &'a StackPass<'a>,
    pub effect: EffectCx<'a>,
}

#[cfg(test)]
mod completeness;
#[cfg(test)]
mod pack_tests;
#[cfg(test)]
mod probe_gpu_tests;
