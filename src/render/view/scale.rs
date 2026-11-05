//! src/render/view/scale.rs — a view's render scale (#1498), Unity URP's *Render
//! Scale*: the scene renders at the output's size times the scale, and the finished,
//! post-processed image is upscaled to the output with bilinear filtering (URP's
//! *Auto* filter outside FSR) before the UI, which stays at the output's resolution.
//! A view running TAA upscales inside the chain instead (TAAU, #1702:
//! `postfx::effects::taa::upscale`), at TAA's place with the rest of the chain at
//! the output's size, and this bilinear copy is then skipped.
//!
//! A view is native until its consumer sets a scale: only the screen's views (the
//! standalone player, the editor's Game view, a harness capture) do, from
//! `Renderer::graphics`, so the Scene view, previews, render textures and cubemap
//! bakes always render at their own size. At the native scale the two sizes match,
//! no upscale target exists and the frame is the one it was before scaling existed.

use super::{PixelSize, RenderView};
use crate::core::settings::render_scale::RenderScale;
use crate::render::postfx::PostFxContext;
use crate::render::timing::GpuPass;
use crate::render::Renderer;

impl RenderView {
    /// Render this view's scene at `scale` of its output from the next resize on (the
    /// next render's, at the latest).
    pub fn set_render_scale(&mut self, scale: RenderScale) {
        self.render_scale = scale;
    }

    /// The scale this view's scene renders at.
    pub fn render_scale(&self) -> RenderScale {
        self.render_scale
    }

    /// Where a render of this view composites its scene when it upscales bilinearly
    /// (`bilinear`): the upscale source, made the first frame it is needed, while the
    /// scene is scaled. `None` (straight into the caller's output) on a native view,
    /// and on a frame TAA upscales (TAAU, #1702), whose chain writes the output
    /// itself: the source is then dropped, so a view under TAAU holds none.
    pub(crate) fn scaled_output(
        &mut self,
        device: &wgpu::Device,
        bilinear: bool,
    ) -> Option<wgpu::TextureView> {
        if !bilinear {
            self.upscale_source = None;
            return None;
        }
        if self.upscale_source.is_none() {
            let (format, size, scene) = (self.format, self.size, self.scene_size);
            self.upscale_source = upscale_source(device, format, size, scene);
        }
        let source = self.upscale_source.as_ref()?;
        Some(source.create_view(&wgpu::TextureViewDescriptor::default()))
    }

    /// Whether this view holds its bilinear upscale source, for a test.
    #[cfg(test)]
    pub(crate) fn holds_upscale_source(&self) -> bool {
        self.upscale_source.is_some()
    }
}

/// `output` at `scale`, each side rounded, at least 1 and at most `max_side` (the
/// device's `max_texture_dimension_2d`): a 5K screen at scale 2 would ask for a texture
/// wider than the device allows, and a script must not be able to crash the engine.
pub(super) fn scene_size(scale: RenderScale, output: PixelSize, max_side: u32) -> PixelSize {
    let side = |px| scale.apply(px).min(max_side.max(1));
    PixelSize::new(side(output.width), side(output.height))
}

/// The upscale source for a view whose scene is `scene` and output `output`, in the
/// output's format, or `None` when the two sizes match.
pub(super) fn upscale_source(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    output: PixelSize,
    scene: PixelSize,
) -> Option<wgpu::Texture> {
    (output != scene).then(|| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("View Scaled Scene"),
            size: wgpu::Extent3d {
                width: scene.width,
                height: scene.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    })
}

impl Renderer {
    /// Upscale `scaled` (the view's post-processed scene, [`RenderView::scaled_output`])
    /// onto `output` with the post-FX chain's bilinear copy, the last scene pass
    /// before the UI.
    pub(crate) fn upscale_scene(
        &self,
        view: &RenderView,
        scaled: &wgpu::TextureView,
        output: &wgpu::TextureView,
    ) {
        let pfx = &view.post_fx;
        let sky = self.sky_panorama();
        let ctx = PostFxContext {
            depth_view: &view.depth_view,
            skybox: (&sky.view, &sky.sampler),
            output,
            timer: &self.gpu_timer,
        };
        // The aux, depth and skybox bindings are unused by the copy but must satisfy
        // the chain's shared IO layout.
        let io = pfx.io_bind_group(
            &self.device,
            scaled,
            pfx.bloom_view(),
            ctx.depth_view,
            ctx.skybox,
        );
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Scale Upscale"),
            });
        let copy = (&pfx.copy_pipeline, ctx.timer.writes(GpuPass::PostFx));
        crate::render::postfx::PostFx::fullscreen(&mut encoder, copy, &[&io], output);
        self.queue.submit(Some(encoder.finish()));
    }
}

#[cfg(test)]
#[path = "scale_tests.rs"]
mod tests;
