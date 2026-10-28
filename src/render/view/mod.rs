//! src/render/view/mod.rs — per-view render state (#355).
//!
//! A *view* is one (scene, camera, target) render per frame. The `Renderer` used to
//! bake in a single-scene/single-view assumption: one `size`, one depth buffer, and
//! one post-FX chain, all shared by every consumer. Two consumers rendering in the
//! same frame — the editor viewport and the Inspector preview — therefore fought over
//! those shared targets: their resize guards permanently defeated each other
//! (reallocating both targets twice per frame) and one view's motion-blur history
//! (`prev_view_proj`) bled into the other.
//!
//! `RenderView` owns that per-view state so N views render independently. Each view
//! carries its own size, depth buffer, post-FX chain (and its temporal history), a
//! cached decal-depth bind group, and — for the editor viewport / Inspector preview —
//! the offscreen colour target it renders into. Consumers own their views (the
//! headless screenshot / cubemap capture build a throwaway one), and the render entry
//! point takes the view, so a view's resize guard checks its *own* size.
//!
//! A view has two sizes (#1498): its **output** ([`RenderView::size`]: the colour
//! target, the UI, the projection's aspect) and its **scene** size
//! ([`RenderView::scene_size`]: depth, the SSAO prepass, motion vectors, every post-FX
//! target), the output at the view's [`RenderScale`]. They differ only on a scaled
//! screen view, whose post-processed scene is upscaled to the output before the UI.

use crate::core::settings::render_scale::RenderScale;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::PostFx;

mod output;
mod scale;
mod size;
mod targets;
pub use size::PixelSize;
use targets::{create_color_target, create_depth};

/// Per-view render targets + post-FX chain. See the module docs for why this is owned
/// per view rather than shared on the `Renderer`.
pub struct RenderView {
    /// The output's size: the colour target, the UI and the projection's aspect.
    size: PixelSize,
    /// The scene targets' size: `size` at `render_scale` (#1498).
    scene_size: PixelSize,
    /// What `scene_size` is of `size`; native until a screen view's consumer sets it.
    render_scale: RenderScale,
    /// The post-processed scene at `scene_size`, upscaled to the output before the UI;
    /// `None` while the two sizes are equal, so a native view draws as it always did.
    upscale_source: Option<wgpu::Texture>,
    /// Colour format this view composites into (the swapchain format for the editor
    /// viewport/preview, the offscreen format for screenshots) — its post-FX composite
    /// pipeline and owned colour target are built against it.
    format: wgpu::TextureFormat,
    /// The quality tier's bloom-buffer divisor currently applied to `post_fx`; a change
    /// (a live quality switch) triggers a re-size the next frame.
    bloom_divisor: u32,
    pub(crate) depth_texture: wgpu::Texture,
    pub(crate) depth_view: wgpu::TextureView,
    /// Post-process chain (bloom, colour correction, motion blur, SSR) + its temporal
    /// history. Per view, so one view's previous frame is never another's motion-blur
    /// reference.
    pub(crate) post_fx: PostFx,
    /// Cached scene-depth bind group the decal and soft-particle passes sample (#440);
    /// references this view's depth view, so it is invalidated (set `None`) whenever
    /// the depth target is reallocated on resize.
    pub(crate) scene_depth_bind_group: Option<wgpu::BindGroup>,
    /// SSAO targets (#436), allocated the first frame this view runs AO; they check
    /// their own size, so `resize` leaves them alone.
    pub(crate) ssao: Option<crate::render::passes::ssao::SsaoTargets>,
    /// The offscreen colour target this view renders into (editor viewport / Inspector
    /// preview), `RENDER_ATTACHMENT | TEXTURE_BINDING` so egui samples it. `None` for a
    /// targetless view whose caller supplies the output and reads it back itself.
    color_target: Option<wgpu::Texture>,
    /// A forward pipeline this view draws solids with instead of the renderer's shared
    /// one (#355 step 4) — the Inspector's Shader-asset preview, which shades its mesh
    /// with the module being previewed.
    ///
    /// This replaces a mutate-and-restore on the shared `Renderer::render_pipeline`:
    /// swapping a field out around a call and putting it back leaves the renderer in
    /// the wrong state if anything between the two returns early or unwinds, and it is
    /// invisible at the call site. Ownership by the view says the same thing
    /// declaratively — *this* view shades this way — and cannot leak to another.
    forward_override: Option<wgpu::RenderPipeline>,
    /// The UI pass's format for the colour target (#418); see `create_color_target`.
    ui_format: Option<wgpu::TextureFormat>,
    /// A targetless view's UI output for the next render (#418) — see
    /// [`RenderView::set_ui_output`]. Consumed by that render.
    ui_output: Option<(wgpu::TextureView, wgpu::TextureFormat)>,
    /// The UI pass's per-canvas vertex buffers for this view (#418).
    pub(crate) ui: crate::render::ui::UiViewCache,
    /// The render textures this view's scene draws (#430); see
    /// `crate::render::render_texture`.
    pub(crate) render_textures: crate::render::render_texture::RenderTextures,
    /// Physical pixels per logical one on the display this view is shown on: what
    /// keeps a width in pixels (the selection outline's) the same on a Retina display.
    pixel_scale: f32,
    /// The selection outline's mask (#1175), allocated the first frame this view
    /// outlines; it checks its own size, so `resize` leaves it alone.
    pub(crate) selection_mask: Option<crate::render::passes::outline::SelectionMask>,
}

impl RenderView {
    /// The forward pipeline this view draws solids with: its own override when set,
    /// else the renderer's shared one.
    pub(crate) fn forward_pipeline<'a>(
        &'a self,
        shared: &'a wgpu::RenderPipeline,
    ) -> &'a wgpu::RenderPipeline {
        self.forward_override.as_ref().unwrap_or(shared)
    }

    /// Whether this view draws its solids with a pipeline of its own.
    pub(crate) fn has_forward_override(&self) -> bool {
        self.forward_override.is_some()
    }

    /// Draw this view's solids with `pipeline` instead of the shared forward pipeline.
    /// Pass `None` to go back to the shared one.
    pub fn set_forward_override(&mut self, pipeline: Option<wgpu::RenderPipeline>) {
        self.forward_override = pipeline;
    }

    /// A view that owns an offscreen colour target — the editor viewport and Inspector
    /// preview render into it and show it as an `egui::Image`.
    pub fn offscreen(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        bloom_divisor: u32,
    ) -> Self {
        Self::build(device, format, width, height, bloom_divisor, true)
    }

    /// A view with no owned colour target: the caller supplies the output view and
    /// reads it back itself (the headless screenshot / cubemap capture allocate their
    /// own `COPY_SRC` target).
    pub fn targetless(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        bloom_divisor: u32,
    ) -> Self {
        Self::build(device, format, width, height, bloom_divisor, false)
    }

    fn build(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        bloom_divisor: u32,
        owns_target: bool,
    ) -> Self {
        let (width, height) = (width.max(1), height.max(1));
        // A throwaway registry compiles this view's post-FX module. Views are created
        // rarely (once per consumer, not per frame), so the compile cost is one-time.
        let mut registry = ShaderRegistry::new(crate::shadergen::engine_shader_dir());
        let post_fx = PostFx::new(device, width, height, format, bloom_divisor, &mut registry);
        let (depth_texture, depth_view) = create_depth(device, width, height);
        let target = owns_target.then(|| create_color_target(device, format, width, height));
        let (color_target, ui_format) = target.unzip();
        Self {
            size: PixelSize::new(width, height),
            scene_size: PixelSize::new(width, height),
            render_scale: RenderScale::NATIVE,
            upscale_source: None,
            format,
            bloom_divisor,
            depth_texture,
            depth_view,
            post_fx,
            scene_depth_bind_group: None,
            ssao: None,
            color_target,
            forward_override: None,
            ui_format,
            ui_output: None,
            ui: Default::default(),
            render_textures: Default::default(),
            pixel_scale: 1.0,
            selection_mask: None,
        }
    }

    /// Re-size this view's output to `width` x `height` and its scene targets to that
    /// at its render scale, at `bloom_divisor`, but only when something actually
    /// changed — a cheap no-op otherwise, so the resize guard checks *this view's* own
    /// size instead of a shared one. The post-FX *pipelines* are never rebuilt; only
    /// the size-dependent textures are reallocated.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32, bloom_divisor: u32) {
        let size = PixelSize::new(width.max(1), height.max(1));
        let scene = scale::scene_size(self.render_scale, size);
        if (self.size, self.scene_size, self.bloom_divisor) == (size, scene, bloom_divisor) {
            return;
        }
        // A scale change alone keeps the output: a consumer may already hold a view of
        // its colour target for this render.
        let output_changed = self.size != size;
        (self.size, self.scene_size, self.bloom_divisor) = (size, scene, bloom_divisor);
        let (depth_texture, depth_view) = create_depth(device, scene.width, scene.height);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
        // The cached scene-depth bind group references the freed depth view.
        self.scene_depth_bind_group = None;
        let (w, h) = (scene.width, scene.height);
        self.post_fx.resize(device, w, h, bloom_divisor);
        self.upscale_source = scale::upscale_source(device, self.format, size, scene);
        if output_changed && self.color_target.is_some() {
            let (w, h) = (size.width, size.height);
            let (target, ui_format) = create_color_target(device, self.format, w, h);
            (self.color_target, self.ui_format) = (Some(target), Some(ui_format));
        }
    }

    /// Set the display's physical pixels per logical one (the window's scale factor);
    /// 1 until set. Widths drawn in pixels, like the selection outline's, scale by it.
    pub fn set_pixel_scale(&mut self, scale: f32) {
        self.pixel_scale = scale.max(1e-3);
    }

    /// The display's physical pixels per logical one ([`Self::set_pixel_scale`]).
    pub fn pixel_scale(&self) -> f32 {
        self.pixel_scale
    }

    /// This view's output size: its colour target's, the UI's, the projection aspect's.
    pub fn size(&self) -> PixelSize {
        self.size
    }

    /// The size of this view's scene targets (depth, SSAO, motion vectors, post-FX):
    /// [`Self::size`] at its render scale (#1498). What a scene pass sizes by.
    pub fn scene_size(&self) -> PixelSize {
        self.scene_size
    }

    /// The projection aspect ratio (`width / height`) of the output, which the scaled
    /// scene is stretched back to.
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
