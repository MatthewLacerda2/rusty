//! src/render/view.rs — per-view render state (#355).
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

/// The depth target's format — a depth buffer sampled by the decal + post-FX passes,
/// so it needs `TEXTURE_BINDING` alongside `RENDER_ATTACHMENT`.
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

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
    /// Cached decal-pass depth bind group; references this view's depth view, so it is
    /// invalidated (set `None`) whenever the depth target is reallocated on resize.
    pub(crate) decal_depth_bind_group: Option<wgpu::BindGroup>,
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
        let mut registry = ShaderRegistry::new("assets/shaders");
        let post_fx = PostFx::new(device, width, height, format, bloom_divisor, &mut registry);
        let (depth_texture, depth_view) = create_depth(device, width, height);
        let color_target = owns_target.then(|| create_color_target(device, format, width, height));
        Self {
            size: winit::dpi::PhysicalSize::new(width, height),
            format,
            bloom_divisor,
            depth_texture,
            depth_view,
            post_fx,
            decal_depth_bind_group: None,
            color_target,
            forward_override: None,
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
        // The cached decal depth bind group references the freed depth view.
        self.decal_depth_bind_group = None;
        self.post_fx.resize(device, width, height, bloom_divisor);
        if self.color_target.is_some() {
            self.color_target = Some(create_color_target(device, self.format, width, height));
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

/// Allocate a depth texture (+ its default view) at `width` x `height`. Sampled by the
/// decal and post-FX passes, hence `TEXTURE_BINDING`.
fn create_depth(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("View Depth Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// Allocate the offscreen colour target: sampled by egui (`TEXTURE_BINDING`) and
/// readable back to the CPU (`COPY_SRC`), so one owned target serves both the editor's
/// `egui::Image` and the dev layer's PNG capture instead of each allocating its own.
fn create_color_target(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("View Colour Target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod view_tests;
