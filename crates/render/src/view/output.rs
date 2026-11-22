use super::RenderView;
use rusty_core::settings::render_scale::RenderScale;

impl RenderView {
    pub fn color_target_view(&self) -> Option<wgpu::TextureView> {
        self.color_target
            .as_ref()
            .map(|t| t.create_view(&wgpu::TextureViewDescriptor::default()))
    }

    pub fn display_view(&self) -> Option<wgpu::TextureView> {
        let target = self.color_target.as_ref()?;
        Some(target.create_view(&wgpu::TextureViewDescriptor {
            format: self.ui_format,
            ..Default::default()
        }))
    }

    pub fn set_ui_output(&mut self, output: Option<(wgpu::TextureView, wgpu::TextureFormat)>) {
        self.ui_output = output;
    }

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

    pub fn color_target(&self) -> Option<&wgpu::Texture> {
        self.color_target.as_ref()
    }

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
