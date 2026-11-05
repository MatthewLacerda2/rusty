//! src/render/draw/camera/stages.rs — the stages [`super::CAMERA_PASSES`] lists, in
//! its order: each a thin call into the feature's own module.

use super::CameraCtx;
use crate::render::draw::pass::{PassClear, ScenePassFrame};
use crate::render::passes::ssao::SsaoFrame;
use crate::render::postfx::effects::ssr;
use crate::render::{RenderView, Renderer};
use crate::scene::ClearFlags;

pub(super) fn upload_camera(r: &mut Renderer, _: &mut RenderView, ctx: &mut CameraCtx) {
    r.queue
        .write_buffer(&r.camera_buffer, 0, bytemuck::bytes_of(&ctx.uniform));
}

/// Bin the frame's point and spot lights (#434) and its decals (#638) into this
/// camera's clusters. An overlay camera ([`is_overlay`]) bins no decals: world
/// decals never land on the viewmodel.
pub(super) fn bin_clusters(r: &mut Renderer, _: &mut RenderView, ctx: &mut CameraCtx) {
    r.bin_clusters(&ctx.clusters, !ctx.overlay);
}

/// Volumetric fog (#1703): the base camera's froxels into the renderer's volume,
/// after its lights are binned and before anything draws (`volumetric_fog::run`).
pub(super) fn volumetric_fog(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    r.run_volumetric_fog(view, ctx);
}

/// This camera's solids (the culling mask differs per camera) as instanced draws
/// (#470), split opaque/cutout vs transparent (#242).
/// A view writing motion vectors gives each solid its last-frame pose (#1435).
pub(super) fn batch_solids(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    let outline = ctx.stack.editor_mode;
    let motion = &mut view.post_fx.motion;
    let history = motion.active().then_some(&mut motion.history);
    let (scene, cam) = (ctx.scene, ctx.cam);
    ctx.solids =
        r.precreate_solid_resources(scene, cam, &ctx.frustum, &ctx.lod, (outline, history));
}

/// A light-probe capture's clear (#1668): black, and no sky, so a direction that
/// escapes the scene carries only the ambient its bake adds.
const OPEN_CLEAR: PassClear = PassClear {
    color: Some(wgpu::Color::BLACK),
    flags: ClearFlags::SolidColor,
};

pub(super) fn scene_pass(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    let frame = ScenePassFrame {
        clear: match r.capture.ambient_sky {
            true => OPEN_CLEAR,
            false => PassClear::for_pass(ctx.is_first, ctx.cam.clear_flags),
        },
        ssao: ctx
            .ssao
            .map(|plan| SsaoFrame::for_camera(plan, ctx.view_proj, ctx.cam)),
        base: ctx.is_first,
        ssil: ctx.ssil,
    };
    r.execute_scene_pass(view, frame, &ctx.solids.draws.opaque);
}

/// SSIL's source (#1750), kept after the base camera's opaque pass (`ssil::run`).
pub(super) fn ssil_source(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    r.keep_ssil_source(view, ctx);
}

/// Screen-space reflections (#1526) over the base camera's opaque image, before
/// anything blends over it or an overlay camera draws: the base camera's alone.
pub(super) fn ssr(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
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

/// Unity's *Opaque Texture* (#1523): this camera's colour and depth, after SSR, for
/// the translucents that refract or absorb, or trace SSR (#1570); skipped when none
/// of them does.
pub(super) fn opaque_copy(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    r.copy_opaque(view, &ctx.solids.draws.transparent, ctx.traces);
}

/// Translucent solids (#242), back-to-front over the opaque ones.
pub(super) fn transparent(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    r.draw_transparent(view, &ctx.solids.draws.transparent);
}

/// HDRP's transparent depth postpass (#1523): the translucents that ask for it
/// write their depth (and velocity texel) over what they drew over.
pub(super) fn depth_postpass(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    r.draw_depth_postpass(view, &ctx.solids.draws.transparent);
}

/// World canvases (#429): scene geometry, occluded by it, before particles.
pub(super) fn world_ui(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    r.draw_world_ui(view, ctx.cam, ctx.view_proj, ctx.fog);
}

/// Ribbons (#441) then sprite particles, each back to front; mesh particles drew
/// with the solids.
pub(super) fn effects(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    ctx.effects = r.draw_effects(view, ctx.scene, ctx.cam, ctx.view_proj);
}

pub(super) fn count(r: &mut Renderer, _: &mut RenderView, ctx: &mut CameraCtx) {
    r.count_camera(&ctx.solids, ctx.effects);
}

/// Composite + post-process once, over the final camera's HDR target.
pub(super) fn post_fx(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    if ctx.is_last {
        r.run_post_fx(view, ctx);
    }
}

/// The editor's selection outline (#1175), onto the finished post-FX image (#1224).
/// Only an editor camera marked any solid for it (`batch_solids`).
pub(super) fn outline(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    if ctx.is_last {
        r.draw_outline(view, &ctx.solids, ctx.view_proj, ctx.stack.output);
    }
}

/// The editor's handles and gizmos (grid, collider boxes, axis arrows, probes) over
/// the outline, depth-tested against this camera's scene (#1224).
pub(super) fn handles(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    if ctx.is_last && ctx.stack.editor_mode {
        r.draw_handles(view, ctx.scene, ctx.stack.output);
    }
}
