//! `RenderView::display_view` (#333): egui samples a texture as the bytes stored, not
//! decoded to linear, so the view the editor hands it must read the sRGB target through
//! its non-sRGB twin. Through the plain sRGB view the viewport shows darker.

use super::RenderView;
use crate::render::{readback, OFFSCREEN_FORMAT};

const SIZE: u32 = 4;
/// Linear mid-grey, cleared through the sRGB view: the target stores it encoded, as 188.
const LINEAR: f64 = 0.5;
const STORED: u8 = 188;

/// Texel (0,0) of the sampled view, copied onto an 8-bit target with no conversion.
const COPY_TEXEL: &str = r"
@group(0) @binding(0) var src: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    return vec4<f32>(p, 0.0, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> {
    return textureLoad(src, vec2<i32>(0, 0), 0);
}
";

/// One pass onto `view`: cleared to `clear` (in the view's own encoding) or loaded.
fn pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    clear: Option<f64>,
) -> wgpu::RenderPass<'a> {
    let load = clear.map_or(wgpu::LoadOp::Load, |v| {
        wgpu::LoadOp::Clear(wgpu::Color {
            r: v,
            g: v,
            b: v,
            a: 1.0,
        })
    });
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

/// A pipeline drawing texel (0,0) of its one bound texture over an 8-bit target.
fn copy_texel_pipeline(device: &wgpu::Device) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(COPY_TEXEL.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// What a shader reading `view` sees at texel (0,0), as an 8-bit red channel.
fn sampled_red(device: &wgpu::Device, queue: &wgpu::Queue, view: &wgpu::TextureView) -> u8 {
    let pipeline = copy_texel_pipeline(device);
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(view),
        }],
    });
    let out = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut p = pass(
            &mut encoder,
            &out.create_view(&Default::default()),
            Some(0.0),
        );
        p.set_pipeline(&pipeline);
        p.set_bind_group(0, &bind, &[]);
        p.draw(0..3, 0..1);
    }
    queue.submit(std::iter::once(encoder.finish()));
    readback::read_texture_rgba8(device, queue, &out, SIZE, SIZE)[0]
}

#[test]
fn gpu_display_view_samples_the_stored_bytes() {
    let Some(r) = crate::render::test_gpu::headless_or_skip(SIZE, SIZE) else {
        return;
    };
    let view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, SIZE, SIZE, 2);
    let srgb = view
        .color_target_view()
        .expect("an offscreen view owns a target");
    let mut encoder = r.device.create_command_encoder(&Default::default());
    drop(pass(&mut encoder, &srgb, Some(LINEAR)));
    r.queue.submit(std::iter::once(encoder.finish()));

    let display = view
        .display_view()
        .expect("an offscreen view owns a target");
    let raw = sampled_red(&r.device, &r.queue, &display);
    assert!(
        raw.abs_diff(STORED) <= 1,
        "display view read {raw}, not the stored {STORED}"
    );
    // The control: the plain sRGB view decodes to linear, which egui must not get.
    let decoded = sampled_red(&r.device, &r.queue, &srgb);
    assert!(decoded.abs_diff(128) <= 1, "the sRGB view read {decoded}");
}
