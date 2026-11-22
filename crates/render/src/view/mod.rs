use crate::gpu::shaders::ShaderRegistry;
use crate::postfx::PostFx;
use rusty_core::settings::render_scale::RenderScale;

mod output;
mod scale;
mod size;
mod targets;
pub use size::PixelSize;
use targets::{create_color_target, create_depth};

pub struct RenderView {
    size: PixelSize,
    scene_size: PixelSize,
    render_scale: RenderScale,
    upscale_source: Option<wgpu::Texture>,
    format: wgpu::TextureFormat,
    pub(crate) depth_texture: wgpu::Texture,
    pub(crate) depth_view: wgpu::TextureView,
    pub(crate) post_fx: PostFx,
    pub(crate) scene_depth_bind_group: Option<wgpu::BindGroup>,
    pub(crate) ssao: Option<crate::passes::ssao::SsaoTargets>,
    pub(crate) hiz: crate::passes::hiz::HiZView,
    color_target: Option<wgpu::Texture>,
    forward_override: Option<crate::gpu::pipelines::FacedPipeline>,
    ui_format: Option<wgpu::TextureFormat>,
    ui_output: Option<(wgpu::TextureView, wgpu::TextureFormat)>,
    pub ui: crate::ui::UiViewCache,
    pub(crate) render_textures: crate::render_texture::RenderTextures,
    pixel_scale: f32,
    pub(crate) selection_mask: Option<crate::passes::outline::SelectionMask>,
    pub gizmos: rusty_core::application::SceneGizmos,
    pub game_gizmos: bool,
    pub(crate) gizmo_depth: Option<(wgpu::Texture, wgpu::TextureView)>,
    pub(crate) gizmo_upscale_told: bool,
}

impl RenderView {
    pub(crate) fn forward_pipeline<'a>(
        &'a self,
        shared: &'a crate::gpu::pipelines::FacedPipeline,
    ) -> &'a crate::gpu::pipelines::FacedPipeline {
        self.forward_override.as_ref().unwrap_or(shared)
    }

    pub(crate) fn has_forward_override(&self) -> bool {
        self.forward_override.is_some()
    }

    pub(crate) fn set_forward_override(
        &mut self,
        pipeline: Option<crate::gpu::pipelines::FacedPipeline>,
    ) {
        self.forward_override = pipeline;
    }

    pub fn offscreen(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        Self::build(device, format, width, height, true)
    }

    pub fn targetless(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        Self::build(device, format, width, height, false)
    }

    fn build(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        owns_target: bool,
    ) -> Self {
        let (width, height) = (width.max(1), height.max(1));
        let mut registry = ShaderRegistry::new(rusty_shadergen::engine_shader_dir());
        let post_fx = PostFx::new(device, width, height, format, &mut registry);
        let (depth_texture, depth_view) = create_depth(device, width, height);
        let target = owns_target.then(|| create_color_target(device, format, width, height));
        let (color_target, ui_format) = target.unzip();
        Self {
            size: PixelSize::new(width, height),
            scene_size: PixelSize::new(width, height),
            render_scale: RenderScale::NATIVE,
            upscale_source: None,
            format,
            depth_texture,
            depth_view,
            post_fx,
            scene_depth_bind_group: None,
            ssao: None,
            hiz: Default::default(),
            color_target,
            forward_override: None,
            ui_format,
            ui_output: None,
            ui: Default::default(),
            render_textures: Default::default(),
            pixel_scale: 1.0,
            selection_mask: None,
            gizmos: Default::default(),
            game_gizmos: false,
            gizmo_depth: None,
            gizmo_upscale_told: false,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let size = PixelSize::new(width.max(1), height.max(1));
        let max_side = device.limits().max_texture_dimension_2d;
        let scene = scale::scene_size(self.render_scale, size, max_side);
        if (self.size, self.scene_size) == (size, scene) {
            return;
        }
        // A scale-only change keeps the output target: a consumer may hold a view of it this
        // frame.
        let output_changed = self.size != size;
        (self.size, self.scene_size) = (size, scene);
        let (depth_texture, depth_view) = create_depth(device, scene.width, scene.height);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
        self.scene_depth_bind_group = None;
        self.ssao = None;
        let (w, h) = (scene.width, scene.height);
        self.post_fx.resize(device, w, h);
        self.upscale_source = None;
        if output_changed && self.color_target.is_some() {
            let (w, h) = (size.width, size.height);
            let (target, ui_format) = create_color_target(device, self.format, w, h);
            (self.color_target, self.ui_format) = (Some(target), Some(ui_format));
        }
    }

    pub fn set_pixel_scale(&mut self, scale: f32) {
        self.pixel_scale = scale.max(1e-3);
    }

    pub fn pixel_scale(&self) -> f32 {
        self.pixel_scale
    }

    pub fn size(&self) -> PixelSize {
        self.size
    }

    pub fn scene_size(&self) -> PixelSize {
        self.scene_size
    }

    pub fn aspect(&self) -> f32 {
        self.size.width as f32 / self.size.height.max(1) as f32
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod view_tests;

#[cfg(test)]
#[path = "display_tests.rs"]
mod display_tests;
