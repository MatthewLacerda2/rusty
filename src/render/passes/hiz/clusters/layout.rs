//! `hiz_clusters.wgsl`'s two bind group layouts (#1968): the camera's (group 0,
//! with each batch's `Run` at a dynamic offset) and a mesh's (group 1).

use crate::render::gpu::layout_entries::{storage_entry, uniform_entry};

/// The stride between batches' `Run`s in their uniform buffer: the dynamic
/// offset alignment every adapter accepts.
pub(crate) const RUN_STRIDE: u64 = 256;
/// `hiz_clusters.wgsl`'s `Run`.
const RUN_BYTES: u64 = 16;

fn storage(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        ..storage_entry(binding, wgpu::ShaderStages::COMPUTE)
    }
}

/// Group 0: params, jobs, objects, visible, state (its first word the count),
/// output indices and args, the pyramid, and the batch's `Run`.
pub(crate) fn cull_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let compute = wgpu::ShaderStages::COMPUTE;
    let pyramid = wgpu::BindGroupLayoutEntry {
        binding: 7,
        visibility: compute,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    let run = wgpu::BindGroupLayoutEntry {
        binding: 8,
        visibility: compute,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: true,
            min_binding_size: wgpu::BufferSize::new(RUN_BYTES),
        },
        count: None,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Cluster Cull"),
        entries: &[
            uniform_entry(0, compute),
            storage(1, true),
            storage(2, true),
            storage(3, true),
            storage(4, false),
            storage(5, false),
            storage(6, false),
            pyramid,
            run,
        ],
    })
}

/// Group 1: a mesh's cluster table and its index buffer.
pub(crate) fn mesh_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Mesh Clusters"),
        entries: &[storage(0, true), storage(1, true)],
    })
}

/// Group 0 over `buffers` (bindings 0 to 6 and 8) and `pyramid` (7).
pub(super) fn cull_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffers: &[&wgpu::Buffer; 8],
    pyramid: &wgpu::TextureView,
) -> wgpu::BindGroup {
    let bindings = [0, 1, 2, 3, 4, 5, 6, 8];
    let mut entries: Vec<_> = bindings
        .iter()
        .zip(buffers)
        .map(|(&binding, buffer)| wgpu::BindGroupEntry {
            binding,
            resource: match binding {
                8 => wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(RUN_BYTES),
                }),
                _ => buffer.as_entire_binding(),
            },
        })
        .collect();
    entries.push(wgpu::BindGroupEntry {
        binding: 7,
        resource: wgpu::BindingResource::TextureView(pyramid),
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Cluster Cull"),
        layout,
        entries: &entries,
    })
}
