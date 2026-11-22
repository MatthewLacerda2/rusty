use crate::gpu::ops::CountedEncoder as _;

pub(super) fn pipeline_layout(
    device: &wgpu::Device,
    groups: &[&wgpu::BindGroupLayout],
) -> wgpu::PipelineLayout {
    let groups: Vec<_> = groups.iter().map(|g| Some(*g)).collect();
    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("SSR Pipeline Layout"),
        bind_group_layouts: &groups,
        immediate_size: 0,
    })
}

pub(super) fn opaque_target(format: wgpu::TextureFormat) -> wgpu::ColorTargetState {
    wgpu::ColorTargetState {
        format,
        blend: None,
        write_mask: wgpu::ColorWrites::ALL,
    }
}

pub(super) type Stamps<'a> = Option<wgpu::RenderPassTimestampWrites<'a>>;

pub(super) fn begin<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    label: &str,
    targets: &[(&'e wgpu::TextureView, wgpu::LoadOp<wgpu::Color>)],
    timestamp_writes: Stamps<'e>,
) -> wgpu::RenderPass<'e> {
    let attachments: Vec<_> = targets
        .iter()
        .map(|&(view, load)| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })
        })
        .collect();
    encoder.counted_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}
