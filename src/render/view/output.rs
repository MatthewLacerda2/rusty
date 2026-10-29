//! src/render/view/output.rs — what a view hands its consumer: the owned colour
//! target (and its display-space alias egui samples), and where the UI pass draws.
//! Always the output's size, never the scene's scaled one (#1498).

use super::RenderView;
use crate::core::settings::render_scale::RenderScale;

impl RenderView {
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
    /// or the render scale (#1498) changes. The scale is set before
    /// the resize, so a frame that changes both size and scale allocates once.
    pub fn ensure_offscreen<'a>(
        slot: &'a mut Option<RenderView>,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        (width, height): (u32, u32),
        render_scale: RenderScale,
    ) -> &'a mut RenderView {
        let view = slot.get_or_insert_with(|| RenderView::offscreen(device, format, width, height));
        view.set_render_scale(render_scale);
        view.resize(device, width, height);
        view
    }
}
