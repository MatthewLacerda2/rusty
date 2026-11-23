//! The stages [`super::CAMERA_PASSES`] lists, in
//! its order: each a thin call into the feature's own module.

use super::CameraCtx;
use crate::draw::pass::{PassClear, ScenePassFrame};
use crate::passes::ssao::SsaoFrame;
use crate::postfx::effects::ssr;
use crate::{RenderView, Renderer};
use rusty_scene::ClearFlags;

pub(super) fn upload_camera(r: &mut Renderer, _: &mut RenderView, ctx: &mut CameraCtx) {
    r.camera_buffer.upload(&r.queue, &ctx.uniform);
}

pub(super) fn bin_clusters(r: &mut Renderer, _: &mut RenderView, ctx: &mut CameraCtx) {
    r.bin_clusters(&ctx.clusters, !ctx.overlay);
}

pub(super) fn volumetric_fog(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    r.run_volumetric_fog(view, ctx);
}

pub(super) fn batch_solids(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    let outline = ctx.stack.editor_mode;
    let order = r.opaque_order(view, ctx.cam, ctx.screen_space());
    let motion = &mut view.post_fx.motion;
    let history = motion.active().then_some(&mut motion.history);
    let (scene, cam) = (ctx.scene, ctx.cam);
    let culled = (&ctx.frustum, &ctx.lod);
    ctx.solids = r.precreate_solid_resources(scene, cam, culled, (outline, history, order));
}

pub(super) fn occlusion(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    r.cull_by_hiz(view, ctx);
}

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
        screen_space: ctx.screen_space(),
        culled: ctx.solids.culled.clone(),
        floored: ctx.solids.floored,
    };
    let encoder = ctx.commands.encoder();
    r.execute_scene_pass(encoder, view, frame, &ctx.solids.draws.opaque);
}

pub(super) fn ssil_source(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    r.keep_ssil_source(view, ctx);
}

pub(super) fn ssr(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    if ctx.is_first {
        let stack = ctx.stack;
        let settings = (
            stack.post,
            r.graphics(stack.editor_mode),
            stack.full_post_fx,
        );
        let at = (ctx.view_proj, ctx.cam.position);
        let rays = r.ssr_ray_groups();
        ssr::run((r, &mut ctx.commands), view, settings, at, rays);
    }
}

pub(super) fn opaque_copy(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    let encoder = ctx.commands.encoder();
    r.copy_opaque(encoder, view, &ctx.solids.draws.transparent, ctx.traces);
}

pub(super) fn transparent(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    r.draw_transparent(ctx.commands.encoder(), view, &ctx.solids.draws.transparent);
}

pub(super) fn depth_postpass(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    let encoder = ctx.commands.encoder();
    r.draw_depth_postpass(encoder, view, &ctx.solids.draws.transparent);
}

pub(super) fn world_ui(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    let encoder = ctx.commands.encoder();
    r.draw_world_ui(encoder, view, ctx.cam, ctx.view_proj, ctx.fog);
}

pub(super) fn effects(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    let camera = (ctx.cam, ctx.uniform.stack[0]);
    let encoder = ctx.commands.encoder();
    ctx.effects = r.draw_effects(encoder, view, ctx.scene, camera, ctx.view_proj);
}

pub(super) fn count(r: &mut Renderer, _: &mut RenderView, ctx: &mut CameraCtx) {
    r.count_camera(&ctx.solids, ctx.effects);
}

pub(super) fn post_fx(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    if ctx.is_last {
        r.run_post_fx(view, ctx);
    }
}

pub(super) fn outline(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    if ctx.is_last {
        let encoder = ctx.commands.encoder();
        r.draw_outline(encoder, view, &ctx.solids, ctx.view_proj, ctx.stack.output);
    }
}

pub(super) fn gizmo_depth(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    if !ctx.is_first {
        return;
    }
    if !game_gizmos(view, ctx) || r.gizmos_upscaled(view, ctx) {
        view.gizmo_depth = None;
    } else if !ctx.is_last {
        r.keep_gizmo_depth(view, ctx);
    }
}

fn game_gizmos(view: &RenderView, ctx: &CameraCtx) -> bool {
    view.game_gizmos && !ctx.stack.editor_mode
}

pub(super) fn handles(r: &mut Renderer, view: &mut RenderView, ctx: &mut CameraCtx) {
    if !ctx.is_last {
        return;
    }
    if ctx.stack.editor_mode {
        let encoder = ctx.commands.encoder();
        r.draw_handles(encoder, view, ctx.scene, ctx.stack.output, ctx.cam.position);
    } else if game_gizmos(view, ctx) {
        r.draw_game_gizmos(view, ctx);
    }
}
