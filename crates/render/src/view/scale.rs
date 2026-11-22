use super::{PixelSize, RenderView};
use crate::postfx::PostFxContext;
use crate::timing::GpuPass;
use crate::Renderer;
use rusty_core::settings::render_scale::RenderScale;

impl RenderView {
    pub fn set_render_scale(&mut self, scale: RenderScale) {
        self.render_scale = scale;
    }

    pub fn render_scale(&self) -> RenderScale {
        self.render_scale
    }

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

    #[cfg(test)]
    pub(crate) fn holds_upscale_source(&self) -> bool {
        self.upscale_source.is_some()
    }
}

pub(super) fn scene_size(scale: RenderScale, output: PixelSize, max_side: u32) -> PixelSize {
    let side = |px| scale.apply(px).min(max_side.max(1));
    PixelSize::new(side(output.width), side(output.height))
}

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
    pub(crate) fn upscale_scene(
        &self,
        encoder: &mut wgpu::CommandEncoder,
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
            passes: &self.post_passes,
        };
        let io = pfx.io_bind_group(
            &self.device,
            scaled,
            pfx.bloom_view(),
            ctx.depth_view,
            ctx.skybox,
        );
        let copy = (&pfx.copy_pipeline, ctx.timer.writes(GpuPass::PostFx));
        crate::postfx::PostFx::fullscreen(encoder, copy, &[&io], output);
    }
}

#[cfg(test)]
#[path = "scale_tests.rs"]
mod tests;
