//! `hiz_clusters.wgsl`'s bind group layouts (#1968): the camera's two group 0s,
//! each with the batch's `Run` at a dynamic offset (the test's and scan's, and
//! the copy's, which leaves out the `tally` its dispatch reads as indirect args,
//! #2406), and a mesh's group 1.

use crate::gpu::layout_entries::{storage_entry, uniform_entry};
use crate::gpu::ops::CountedDevice as _;

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

/// The batch's `Run`, at a dynamic offset.
fn run_entry() -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: 8,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: true,
            min_binding_size: wgpu::BufferSize::new(RUN_BYTES),
        },
        count: None,
    }
}

/// The test's and scan's group 0: params, jobs, objects, visible, state, the
/// tally (the count, then each batch's copy dispatch), output args, the
/// pyramid, and the batch's `Run`.
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
            run_entry(),
        ],
    })
}

/// The copy's group 0: jobs, state, the batch's `Run` and the output indices.
pub(crate) fn copy_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Cluster Copy"),
        entries: &[
            storage(1, true),
            storage(4, false),
            run_entry(),
            storage(9, false),
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

/// A group 0 over `buffers` at `bindings` (the `Run` at 8) and, for the
/// test's and scan's, `pyramid` (7).
pub(super) fn cull_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    (bindings, buffers): (&[u32], &[&wgpu::Buffer]),
    pyramid: Option<&wgpu::TextureView>,
) -> wgpu::BindGroup {
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
    entries.extend(pyramid.map(|view| wgpu::BindGroupEntry {
        binding: 7,
        resource: wgpu::BindingResource::TextureView(view),
    }));
    device.counted_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Cluster Cull"),
        layout,
        entries: &entries,
    })
}
