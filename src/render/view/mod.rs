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

use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::PostFx;

mod targets;
use targets::{create_color_target, create_depth};

/// Per-view render targets + post-FX chain. See the module docs for why this is owned
/// per view rather than shared on the `Renderer`.
pub struct RenderView {
    size: winit::dpi::PhysicalSize<u32>,
    /// Colour format this view composites into (the swapchain format for the editor
    /// viewport/preview, the offscreen format for screenshots) — its post-FX composite
    /// pipeline and owned colour target are built against it.
    format: wgpu::TextureFormat,
    /// The quality tier's bloom-buffer divisor currently applied to `post_fx`; a change
    /// (a live quality switch) triggers a re-size the next frame.
    bloom_divisor: u32,
    depth_texture: wgpu::Texture,
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
        let width = width.max(1);
        let height = height.max(1);
        // A throwaway registry compiles this view's post-FX module. Views are created
        // rarely (once per consumer, not per frame), so the compile cost is one-time.
        let mut registry = ShaderRegistry::new(crate::shadergen::engine_shader_dir());
        let post_fx = PostFx::new(device, width, height, format, bloom_divisor, &mut registry);
        let (depth_texture, depth_view) = create_depth(device, width, height);
        let target = owns_target.then(|| create_color_target(device, format, width, height));
        let (color_target, ui_format) = target.unzip();
        Self {
            size: winit::dpi::PhysicalSize::new(width, height),
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
        }
    }

    /// Re-size this view's targets to `width` x `height` at `bloom_divisor`, but only
    /// when something actually changed — a cheap no-op otherwise, so the resize guard
    /// checks *this view's* own size instead of a shared one. The post-FX *pipelines*
    /// are never rebuilt; only the size-dependent textures are reallocated.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32, bloom_divisor: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.size.width == width
            && self.size.height == height
            && self.bloom_divisor == bloom_divisor
        {
            return;
        }
        self.size = winit::dpi::PhysicalSize::new(width, height);
        self.bloom_divisor = bloom_divisor;
        let (depth_texture, depth_view) = create_depth(device, width, height);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
        // The cached scene-depth bind group references the freed depth view.
        self.scene_depth_bind_group = None;
        self.post_fx.resize(device, width, height, bloom_divisor);
        if self.color_target.is_some() {
            let (target, ui_format) = create_color_target(device, self.format, width, height);
            (self.color_target, self.ui_format) = (Some(target), Some(ui_format));
        }
    }

    /// This view's pixel size — drives the projection aspect and the target sizes.
    pub fn size(&self) -> winit::dpi::PhysicalSize<u32> {
        self.size
    }

    /// The projection aspect ratio (`width / height`) for this view.
    pub fn aspect(&self) -> f32 {
        self.size.width as f32 / self.size.height.max(1) as f32
    }

    /// An owned view of the offscreen colour target, or `None` for a targetless view.
    /// `wgpu::TextureView` is a refcounted handle, so the returned view doesn't borrow
    /// `self` — the front-end can pass it back to `render` as the output and register
    /// it with egui in the same frame.
    pub fn color_target_view(&self) -> Option<wgpu::TextureView> {
        self.color_target
            .as_ref()
            .map(|t| t.create_view(&wgpu::TextureViewDescriptor::default()))
    }

    /// The owned colour target seen in display space — its non-sRGB twin, which holds
    /// the display-encoded bytes — for a sampler that expects those rather than linear
    /// values: egui's (#333). A device without view-format support gets the plain view
    /// (the image then shows darker, as the UI pass's fallback blends off).
    pub fn display_view(&self) -> Option<wgpu::TextureView> {
        let target = self.color_target.as_ref()?;
        Some(target.create_view(&wgpu::TextureViewDescriptor {
            format: self.ui_format,
            ..Default::default()
        }))
    }

    /// Have the next render draw the in-game UI through `output` (a view of the
    /// frame the caller passes to `render`, and its format) — for a targetless view
    /// that presents, like the standalone player's swapchain (#418). Views owning a
    /// target need not: the UI draws onto their own target. A targetless view left
    /// without one (the cubemap capture) never draws UI.
    pub fn set_ui_output(&mut self, output: Option<(wgpu::TextureView, wgpu::TextureFormat)>) {
        self.ui_output = output;
    }

    /// The view + format the UI pass draws through this render (#418): the owned
    /// target's display-space alias (see `targets::create_color_target`), else the
    /// caller's [`RenderView::set_ui_output`], taken so a stale frame is never reused.
    pub(crate) fn take_ui_target(&mut self) -> Option<(wgpu::TextureView, wgpu::TextureFormat)> {
        let Some(target) = self.color_target.as_ref() else {
            return self.ui_output.take();
        };
        let format = self.ui_format?;
        let view = target.create_view(&wgpu::TextureViewDescriptor {
            format: Some(format),
            ..Default::default()
        });
        Some((view, format))
    }

    /// The offscreen colour target itself, or `None` for a targetless view. Borrowed
    /// rather than cloned because the copy-back path ([`crate::render::readback`]) needs
    /// the `Texture`, not a view of it — that is what lets a capture read back the very
    /// target it just drew instead of allocating a second one beside it.
    pub fn color_target(&self) -> Option<&wgpu::Texture> {
        self.color_target.as_ref()
    }

    /// Create the offscreen view in `slot` if absent, else resize it in place — so a
    /// consumer keeps one view across frames and only reallocates when the panel size
    /// or quality tier changes.
    pub fn ensure_offscreen(
        slot: &mut Option<RenderView>,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        bloom_divisor: u32,
    ) {
        match slot {
            Some(view) => view.resize(device, width, height, bloom_divisor),
            None => {
                *slot = Some(RenderView::offscreen(
                    device,
                    format,
                    width,
                    height,
                    bloom_divisor,
                ))
            }
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod view_tests;

#[cfg(test)]
#[path = "display_tests.rs"]
mod display_tests;
