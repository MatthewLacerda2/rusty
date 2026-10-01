//! src/render/draw/camera.rs — the per-camera pass list (#636). Every camera of a
//! stack runs [`CAMERA_PASSES`] in order over one [`CameraCtx`], built once by
//! [`CameraCtx::new`]. The order is the table's order, reviewable in one place, the
//! same way `app/registry.rs` orders the sim systems.
//!
//! **Adding a render stage:** write a `fn(&mut Renderer, &mut RenderView,
//! &mut CameraCtx)` (usually a one-line call into the feature's own module, taking
//! what it needs from the context) and add one `(name, fn)` row to [`CAMERA_PASSES`]
//! where it belongs in the frame. A value the stage needs from every camera goes in
//! [`CameraCtx`]; a new camera-uniform field is set once, in [`CameraCtx::new`].
//! No trait objects and no registration at runtime: a static, ordered table.
//!
//! One stage is not every camera's: `post_fx` runs only on the stack's last camera
//! (it composites the whole stack) and returns early on the others.

use super::pass::{PassClear, ScenePassFrame};
use super::resources::SolidResources;
use super::stack::StackPass;
use crate::render::gpu::uniforms::FogUniform;
use crate::render::lod::LodSelection;
use crate::render::passes::particles::ParticleDraws;
use crate::render::passes::ssao::{SsaoFrame, SsaoPlan};
use crate::render::{CameraUniform, Frustum, RenderView, Renderer};
use crate::scene::{Camera, Scene};

/// One stage of a camera's frame.
pub(crate) type CameraPass = fn(&mut Renderer, &mut RenderView, &mut CameraCtx);

/// The per-camera stages, in draw order. Append a feature's stage here (see the
/// module doc); the name is for tests and reading, not lookup.
pub(crate) const CAMERA_PASSES: &[(&str, CameraPass)] = &[
    ("camera_uniform", upload_camera),
    ("solids", batch_solids),
    ("scene", scene_pass),
    ("decals", decals),
    ("transparent", transparent),
    ("world_ui", world_ui),
    ("effects", effects),
    ("count", count),
    ("post_fx", post_fx),
];

/// Everything one camera's stages share, built once per camera.
pub(crate) struct CameraCtx<'a> {
    pub scene: &'a Scene,
    pub cam: &'a Camera,
    pub stack: &'a StackPass<'a>,
    /// This camera's place in the stack: the first clears, the last runs post-FX.
    pub is_first: bool,
    pub is_last: bool,
    pub aspect: f32,
    pub view_proj: glam::Mat4,
    /// The stack's base (first) camera's view-projection, for post-FX history.
    pub base_view_proj: glam::Mat4,
    /// World-space frustum, for culling off-screen entities (#330).
    pub frustum: Frustum,
    pub fog: FogUniform,
    pub ssao: Option<SsaoPlan>,
    /// The LOD level each entity shows to this camera (#472).
    pub lod: LodSelection,
    pub uniform: CameraUniform,
    /// Filled by the `solids` stage; empty before it.
    pub solids: SolidResources,
    /// Filled by the `effects` stage, for the counters.
    pub effects: ParticleDraws,
}

/// The per-stack values every camera's context reads.
pub(crate) struct StackShared {
    pub aspect: f32,
    pub base_view_proj: glam::Mat4,
    pub fog: FogUniform,
    pub ssao: Option<SsaoPlan>,
}

impl<'a> CameraCtx<'a> {
    pub(crate) fn new(
        scene: &'a Scene,
        stack: &'a StackPass<'a>,
        shared: &StackShared,
        idx: usize,
    ) -> Self {
        let cam = &stack.stack[idx];
        let view_proj = cam.build_view_projection(shared.aspect);
        Self {
            scene,
            cam,
            stack,
            is_first: idx == 0,
            is_last: idx + 1 == stack.stack.len(),
            aspect: shared.aspect,
            view_proj,
            base_view_proj: shared.base_view_proj,
            frustum: Frustum::from_view_proj(view_proj),
            fog: shared.fog,
            ssao: shared.ssao,
            lod: LodSelection::for_camera(scene, cam),
            uniform: CameraUniform {
                view_proj: view_proj.to_cols_array(),
                camera_pos: cam.position.to_array(),
                time: scene.shader_time,
                fog: shared.fog,
            },
            solids: SolidResources::default(),
            effects: ParticleDraws::default(),
        }
    }
}

fn upload_camera(r: &mut Renderer, _: &mut RenderView, ctx: &mut CameraCtx) {
    r.queue
        .write_buffer(&r.camera_buffer, 0, bytemuck::bytes_of(&ctx.uniform));
}

/// This camera's solids (the culling mask differs per camera) as instanced draws
/// (#470), split opaque/cutout vs transparent (#242).
fn batch_solids(r: &mut Renderer, _: &mut RenderView, ctx: &mut CameraCtx) {
    ctx.solids = r.precreate_solid_resources(ctx.scene, ctx.cam, &ctx.frustum, &ctx.lod);
}

fn scene_pass(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    let overlays = r.precreate_overlays(ctx.scene, ctx.stack.editor_mode);
    let frame = ScenePassFrame {
        editor_mode: ctx.stack.editor_mode,
        clear: PassClear::for_pass(ctx.is_first, ctx.cam.clear_flags),
        ssao: ctx
            .ssao
            .map(|plan| SsaoFrame::for_camera(plan, ctx.view_proj, ctx.cam)),
    };
    r.execute_scene_pass(view, frame, &ctx.solids.draws.opaque, &overlays);
}

/// Decals over the lit surfaces, before transparents and particles so they sit on
/// the surface, not over the sparks.
fn decals(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    r.draw_decals(view, ctx.scene, ctx.cam);
}

/// Translucent solids (#242), back-to-front over opaque + decals.
fn transparent(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    r.draw_transparent(view, &ctx.solids.draws.transparent);
}

/// World canvases (#429): scene geometry, occluded by it, before particles.
fn world_ui(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    r.draw_world_ui(view, ctx.cam, ctx.aspect, ctx.fog);
}

/// Ribbons (#441) then sprite particles, each back to front; mesh particles drew
/// with the solids.
fn effects(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    ctx.effects = r.draw_effects(view, ctx.scene, ctx.cam);
}

fn count(r: &mut Renderer, _: &mut RenderView, ctx: &mut CameraCtx) {
    r.count_camera(&ctx.solids, ctx.scene.decals.len(), ctx.effects);
}

/// Composite + post-process once, over the final camera's HDR target.
fn post_fx(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    if ctx.is_last {
        r.run_post_fx(view, ctx);
    }
}

#[cfg(test)]
#[path = "camera_tests.rs"]
mod tests;
