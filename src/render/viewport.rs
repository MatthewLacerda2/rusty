//! src/render/viewport.rs — the editor's offscreen scene target (#183).
//!
//! The windowed editor no longer renders the 3D scene straight to the swapchain.
//! Instead the scene draws into an offscreen colour texture sized to the editor's
//! *viewport rect* (the central panel between the docked egui panels), which is then
//! shown inside an `egui::Image` so the viewport can live in a Scene/Game tab and be
//! sized/picked against. egui paints the panels over the swapchain as before.
//!
//! `resize_viewport` re-sizes the scene's internal targets (depth + post-FX + the
//! offscreen colour target) and `self.size` (which drives the projection aspect) to
//! the viewport's pixel size, leaving the window surface/swapchain at window size.
//! The headless screenshot path keeps using `resize`, so this is windowed-only.

use super::Renderer;

impl Renderer {
    /// An owned view of the offscreen scene target the editor renders into, or `None`
    /// before the first [`Self::resize_viewport`]. `wgpu::TextureView` is a refcounted
    /// handle, so the returned view doesn't borrow `self` — letting the front-end pass
    /// it back to `render(&mut self, …)` as the post-FX output and register it with
    /// `egui_wgpu` in the same frame.
    pub fn viewport_target_view(&self) -> Option<wgpu::TextureView> {
        self.viewport_target
            .as_ref()
            .map(|t| t.create_view(&wgpu::TextureViewDescriptor::default()))
    }

    /// Resize every scene-internal target (offscreen colour + depth + post-FX) and
    /// the projection aspect to `width` x `height` — the editor viewport's pixel
    /// size. The window surface/swapchain is untouched (egui still paints the panels
    /// over the full window). A cheap no-op when the size is unchanged.
    pub fn resize_viewport(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.size.width == width && self.size.height == height && self.viewport_target.is_some()
        {
            return;
        }

        self.size = winit::dpi::PhysicalSize::new(width, height);

        // The scene's depth + post-FX run at the viewport size, so a smaller panel
        // renders fewer pixels rather than stretching a window-sized image.
        let depth_config = wgpu::SurfaceConfiguration {
            width,
            height,
            ..self.config.clone()
        };
        let (depth_tex, depth_view) = Self::create_depth_resources(&self.device, &depth_config);
        self.depth_texture = depth_tex;
        self.depth_view = depth_view;
        self.decal_depth_bind_group = None;
        self.post_fx
            .resize(&self.device, width, height, self.quality.bloom_divisor());

        // The colour target the post-FX writes into; sampled by egui. Its format
        // matches the swapchain so the registered egui texture renders 1:1.
        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Editor Viewport Target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.viewport_target = Some(target);
    }
}
