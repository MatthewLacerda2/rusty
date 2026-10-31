//! Reading a cube texture as a shader does, for `texture_cube_tests` (#1615): each
//! tap is one `textureSampleLevel` of a `texture_cube` view.

use glam::Vec3;

use crate::render::Renderer;

/// What `cube` returns along each `(direction, lod)`, through a `texture_cube`.
pub(super) fn sample(r: &Renderer, cube: &wgpu::Texture, taps: &[(Vec3, f32)]) -> Vec<[u8; 4]> {
    let device = &r.device;
    let view = cube.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
    let dirs = taps_buffer(device, taps);
    let out = out_texture(device, taps.len() as u32);
    let pipeline = pipeline(device);
    let out_view = out.create_view(&Default::default());
    let entries = [
        wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&view),
        },
        wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::Sampler(&sampler),
        },
        wgpu::BindGroupEntry {
            binding: 2,
            resource: dirs.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: 3,
            resource: wgpu::BindingResource::TextureView(&out_view),
        },
    ];
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &entries,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(taps.len() as u32, 1, 1);
    }
    r.queue.submit([encoder.finish()]);
    let bytes =
        crate::render::readback::read_texture_rgba8(device, &r.queue, &out, taps.len() as u32, 1);
    bytes
        .chunks_exact(4)
        .map(|c| [c[0], c[1], c[2], c[3]])
        .collect()
}

/// Each tap as a `vec4` (direction, lod) in a storage buffer.
fn taps_buffer(device: &wgpu::Device, taps: &[(Vec3, f32)]) -> wgpu::Buffer {
    let words: Vec<f32> = taps.iter().flat_map(|(d, l)| [d.x, d.y, d.z, *l]).collect();
    wgpu::util::DeviceExt::create_buffer_init(
        device,
        &wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&words),
            usage: wgpu::BufferUsages::STORAGE,
        },
    )
}

/// The `width × 1` RGBA8 texture the taps are stored into, copyable back.
fn out_texture(device: &wgpu::Device, width: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

/// The compute pipeline running [`SHADER`], its layout derived from the shader.
fn pipeline(device: &wgpu::Device) -> wgpu::ComputePipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    })
}

const SHADER: &str = "
@group(0) @binding(0) var cube: texture_cube<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<storage, read> taps: array<vec4<f32>>;
@group(0) @binding(3) var out: texture_storage_2d<rgba8unorm, write>;
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let tap = taps[id.x];
    textureStore(out, vec2<i32>(i32(id.x), 0), textureSampleLevel(cube, samp, tap.xyz, tap.w));
}";
