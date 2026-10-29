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
//! Four stages are not every camera's: `ssr` runs only on the stack's base camera,
//! over its opaque image (#1526); `post_fx` runs only on the stack's last
//! camera (it composites the whole stack) and returns early on the others, and the
//! editor's `outline` and `handles` follow it there, onto the finished image, in
//! Unity's order: scene, post-FX, outline, handles, then the UI (#1224). An
//! overlay camera ([`is_overlay`], a viewmodel) runs every stage but skips the
//! base camera's screen-space work: no decals in its clusters, no SSAO (#1497).

use super::pass::{PassClear, ScenePassFrame};
use super::resources::SolidResources;
use super::stack::StackPass;
use crate::render::clusters::ClusterGrid;
use crate::render::gpu::uniforms::FogUniform;
use crate::render::lod::LodSelection;
use crate::render::passes::particles::ParticleDraws;
use crate::render::passes::ssao::{SsaoFrame, SsaoPlan};
use crate::render::postfx::effects::ssr;
use crate::render::{CameraUniform, Frustum, RenderView, Renderer};
use crate::scene::{Camera, ClearFlags, Scene};

/// One stage of a camera's frame.
pub(crate) type CameraPass = fn(&mut Renderer, &mut RenderView, &mut CameraCtx);

/// The per-camera stages, in draw order. Append a feature's stage here (see the
/// module doc); the name is for tests and reading, not lookup.
pub(crate) const CAMERA_PASSES: &[(&str, CameraPass)] = &[
    ("camera_uniform", upload_camera),
    ("clusters", bin_clusters),
    ("solids", batch_solids),
    ("scene", scene_pass),
    ("ssr", ssr),
    ("transparent", transparent),
    ("world_ui", world_ui),
    ("effects", effects),
    ("count", count),
    ("post_fx", post_fx),
    ("outline", outline),
    ("handles", handles),
];

/// Everything one camera's stages share, built once per camera.
pub(crate) struct CameraCtx<'a> {
    pub scene: &'a Scene,
    pub cam: &'a Camera,
    pub stack: &'a StackPass<'a>,
    /// This camera's place in the stack: the first clears, the last runs post-FX.
    pub is_first: bool,
    pub is_last: bool,
    /// A `DepthOnly` camera stacked on another (a viewmodel): see [`is_overlay`].
    pub overlay: bool,
    pub aspect: f32,
    pub view_proj: glam::Mat4,
    /// The stack's base (first) camera's view-projection, for post-FX.
    pub base_view_proj: glam::Mat4,
    /// World-space frustum, for culling off-screen entities (#330).
    pub frustum: Frustum,
    /// This camera's light-cluster grid (#434).
    pub clusters: ClusterGrid,
    pub fog: FogUniform,
    /// The stack's SSAO, or `None` on an overlay camera, which samples no AO.
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
    /// Each camera's view-projection, bottom to top, and its last frame's (#1435).
    pub view_projs: Vec<glam::Mat4>,
    pub prev_view_projs: Vec<glam::Mat4>,
    /// The game time the view drew at last frame (#1524), the shader time without one.
    pub prev_time: f32,
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
        let view_proj = shared.view_projs[idx];
        let clusters = ClusterGrid::new(cam, shared.aspect);
        let overlay = is_overlay(idx == 0, cam.clear_flags);
        Self {
            scene,
            cam,
            stack,
            is_first: idx == 0,
            is_last: idx + 1 == stack.stack.len(),
            overlay,
            aspect: shared.aspect,
            view_proj,
            base_view_proj: shared.view_projs[0],
            frustum: Frustum::from_view_proj(view_proj),
            fog: shared.fog,
            ssao: shared.ssao.filter(|_| !overlay),
            lod: LodSelection::for_camera(scene, cam),
            uniform: CameraUniform {
                view_proj: view_proj.to_cols_array(),
                camera_pos: cam.position.to_array(),
                time: scene.shader_time,
                fog: shared.fog,
                clusters: clusters.uniform(),
                prev_view_proj: shared.prev_view_projs[idx].to_cols_array(),
                stack: [idx as f32, shared.prev_time, 0.0, 0.0],
            },
            clusters,
            solids: SolidResources::default(),
            effects: ParticleDraws::default(),
        }
    }
}

/// Whether a camera is an overlay: `DepthOnly` and stacked on another (a viewmodel,
/// a HUD camera). An overlay draws over its base camera's image and shares its
/// screen-space work: it bins no decals and runs no SSAO of its own (#1497, Unity
/// URP's rule: SSAO is a base-camera renderer feature).
pub(crate) fn is_overlay(is_first: bool, clear_flags: ClearFlags) -> bool {
    !is_first && clear_flags == ClearFlags::DepthOnly
}

fn upload_camera(r: &mut Renderer, _: &mut RenderView, ctx: &mut CameraCtx) {
    r.queue
        .write_buffer(&r.camera_buffer, 0, bytemuck::bytes_of(&ctx.uniform));
}

/// Bin the frame's point and spot lights (#434) and its decals (#638) into this
/// camera's clusters. An overlay camera ([`is_overlay`]) bins no decals: world
/// decals never land on the viewmodel.
fn bin_clusters(r: &mut Renderer, _: &mut RenderView, ctx: &mut CameraCtx) {
    r.bin_clusters(&ctx.clusters, !ctx.overlay);
}

/// This camera's solids (the culling mask differs per camera) as instanced draws
/// (#470), split opaque/cutout vs transparent (#242).
/// A view writing motion vectors gives each solid its last-frame pose (#1435).
fn batch_solids(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    let outline = ctx.stack.editor_mode;
    let motion = &mut view.post_fx.motion;
    let history = motion.active().then_some(&mut motion.history);
    let (scene, cam) = (ctx.scene, ctx.cam);
    ctx.solids =
        r.precreate_solid_resources(scene, cam, &ctx.frustum, &ctx.lod, (outline, history));
}

fn scene_pass(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    let frame = ScenePassFrame {
        clear: PassClear::for_pass(ctx.is_first, ctx.cam.clear_flags),
        ssao: ctx
            .ssao
            .map(|plan| SsaoFrame::for_camera(plan, ctx.view_proj, ctx.cam)),
    };
    r.execute_scene_pass(view, frame, &ctx.solids.draws.opaque);
}

/// Screen-space reflections (#1526) over the base camera's opaque image, before
/// anything blends over it or an overlay camera draws: the base camera's alone.
fn ssr(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    if ctx.is_first {
        let stack = ctx.stack;
        let settings = (
            stack.post,
            r.graphics(stack.editor_mode),
            stack.full_post_fx,
        );
        ssr::run(r, view, settings, (ctx.view_proj, ctx.cam.position));
    }
}

/// Translucent solids (#242), back-to-front over the opaque ones.
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
    r.count_camera(&ctx.solids, ctx.effects);
}

/// Composite + post-process once, over the final camera's HDR target.
fn post_fx(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    if ctx.is_last {
        r.run_post_fx(view, ctx);
    }
}

/// The editor's selection outline (#1175), onto the finished post-FX image (#1224).
/// Only an editor camera marked any solid for it (`batch_solids`).
fn outline(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    if ctx.is_last {
        r.draw_outline(view, &ctx.solids, ctx.view_proj, ctx.stack.output);
    }
}

/// The editor's handles and gizmos (grid, collider boxes, axis arrows, probes) over
/// the outline, depth-tested against this camera's scene (#1224).
fn handles(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    if ctx.is_last && ctx.stack.editor_mode {
        r.draw_handles(view, ctx.scene, ctx.stack.output);
    }
}

#[cfg(test)]
#[path = "camera_tests.rs"]
mod tests;
