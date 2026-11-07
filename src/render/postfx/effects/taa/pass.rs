//! The resolve's render pass: one fullscreen triangle into the history's colour
//! and depth frames at once (`ResolveOut` in `postfx/taa.wgsl`), so next frame
//! reads both from the same resolve (#1846).

use super::history::DEPTH_FORMAT;
use crate::render::postfx::run::Timed;
use crate::render::postfx::HDR_FORMAT;

/// What the resolve's pipelines write: the colour frame, then its depths.
pub(super) fn targets() -> [Option<wgpu::ColorTargetState>; 2] {
    let target = |format| wgpu::ColorTargetState {
        format,
        blend: None,
        write_mask: wgpu::ColorWrites::ALL,
    };
    [Some(target(HDR_FORMAT)), Some(target(DEPTH_FORMAT))]
}

/// Draw the resolve, `group` bound at 0, into `[colour, depth]`.
pub(super) fn resolve(
    encoder: &mut wgpu::CommandEncoder,
    (pipeline, timestamp_writes): Timed<'_>,
    group: &wgpu::BindGroup,
    into: [&wgpu::TextureView; 2],
) {
    let attachment = |view| {
        Some(wgpu::RenderPassColorAttachment {
            depth_slice: None,
            view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })
    };
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        multiview_mask: None,
        label: Some("TAA Resolve Pass"),
        color_attachments: &[attachment(into[0]), attachment(into[1])],
        depth_stencil_attachment: None,
        timestamp_writes,
        occlusion_query_set: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.draw(0..3, 0..1);
}
