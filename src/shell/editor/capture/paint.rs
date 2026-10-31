//! Painting the captured egui frame into an offscreen target (#731), shared with the
//! project picker's capture.

use crate::render::OFFSCREEN_FORMAT;

/// Tessellate `output` and paint it over a cleared `width × height` offscreen target.
pub(in crate::shell::editor) fn paint(
    renderer: &crate::render::Renderer,
    egui_renderer: &mut egui_wgpu::Renderer,
    ctx: &egui::Context,
    output: egui::FullOutput,
    width: u32,
    height: u32,
) -> wgpu::Texture {
    let (device, queue) = (&renderer.device, &renderer.queue);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Editor Capture Target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OFFSCREEN_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&Default::default());
    let jobs = ctx.tessellate(output.shapes, output.pixels_per_point);
    let screen = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [width, height],
        pixels_per_point: output.pixels_per_point,
    };
    let mut encoder = device.create_command_encoder(&Default::default());
    egui_renderer.update_buffers(device, queue, &mut encoder, &jobs, &screen);
    {
        let mut pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                multiview_mask: None,
                label: Some("Editor Capture Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: &target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            })
            .forget_lifetime();
        egui_renderer.render(&mut pass, &jobs, &screen);
    }
    queue.submit(std::iter::once(encoder.finish()));
    target
}
