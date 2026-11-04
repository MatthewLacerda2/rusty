//! TAAU (#1702, part 3): TAA's resolve upscaling a scaled view's finished scene to
//! its output, in place of the bilinear upscale (`Renderer::upscale_scene`).
//!
//! **Where it runs: after the post chain, before the UI.** The chain (depth of
//! field, motion blur, the exposure meter, bloom, the grade, the authored effects)
//! runs at the scene's size exactly as it does without TAA, FXAA skipped; this pass
//! then reads its scene-size, display-referred result and resolves it into the
//! output-size history, which a copy lands in the output. So every post pass keeps
//! the cost a render scale buys it, and none learns a second size. (URP's STP, and
//! HDRP's DLSS and FSR2, upscale before the post chain instead and run it at the
//! output's size; here that costs the chain the pixels the scale saved. The PR
//! that brought this, #1702 part 3, measured both.)
//!
//! **The reconstruction** (`fs_upscale` in `postfx/taa.wgsl`): each output pixel
//! gathers the 3×3 scene pixels around it, each weighted by its jittered sample's
//! distance to the output pixel's centre (URP's `CalculateFilterWeights`, a
//! Gaussian fit of Blackman-Harris), blends that into the history as the scene
//! resolve does, with the frame's weight scaled by how near the nearest sample
//! landed, so a sample far from the pixel only nudges it.

use super::{TaaHistory, TaaParams};
use crate::render::postfx::{PostFx, PostFxContext};
use crate::render::timing::GpuPass;
use crate::render::{RenderView, Renderer};

impl Renderer {
    /// Upscale `scaled` (the view's post-processed scene, `RenderView::scaled_output`)
    /// onto `output` through TAA when the view upscales through it this frame, and
    /// say whether it did; `false` leaves the bilinear upscale to the caller.
    pub(crate) fn taa_upscale(
        &self,
        view: &mut RenderView,
        scaled: &wgpu::TextureView,
        output: &wgpu::TextureView,
    ) -> bool {
        let pfx = &view.post_fx;
        let (Some(h), Some(velocity)) = (pfx.targets.get::<TaaHistory>(), pfx.motion.target_view())
        else {
            return false;
        };
        if !h.upscale {
            return false;
        }
        let passes = &self.post_passes.taa;
        let scene = view.scene_size();
        let (read, write) = (h.latest, 1 - h.latest);
        let params = TaaParams::new(h.size, (scene.width, scene.height), h.valid, h.pixels);
        self.queue
            .write_buffer(&h.params, 0, bytemuck::bytes_of(&params));
        let group = passes.group(&self.device, h, read, scaled, velocity);
        let sky = self.sky_panorama();
        let ctx = PostFxContext {
            depth_view: &view.depth_view,
            skybox: (&sky.view, &sky.sampler),
            output,
            timer: &self.gpu_timer,
        };
        // The copy into the output binds the chain's io layout; only its colour is read.
        let resolved = &h.frames[write].view;
        let io = pfx.io_bind_group(
            &self.device,
            resolved,
            pfx.bloom_view(),
            ctx.depth_view,
            ctx.skybox,
        );
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("TAA Upscale Encoder"),
            });
        let timed = (passes.upscale.get(), ctx.timer.writes(GpuPass::Taa));
        PostFx::fullscreen(&mut encoder, timed, &[&group], resolved);
        let copy = (&pfx.copy_pipeline, ctx.timer.writes(GpuPass::PostFx));
        PostFx::fullscreen(&mut encoder, copy, &[&io], output);
        self.queue.submit(Some(encoder.finish()));
        let h = view
            .post_fx
            .targets
            .get_mut::<TaaHistory>()
            .expect("held above");
        (h.latest, h.valid) = (write, true);
        true
    }
}

#[cfg(test)]
mod ghost_gpu_tests;
#[cfg(test)]
mod gpu_tests;
