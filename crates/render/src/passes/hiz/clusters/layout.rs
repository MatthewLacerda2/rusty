//! `hiz_clusters.wgsl`'s bind group layouts (#1968): a group 0 per stage, each
//! binding only what its stage reads (the test's reach the most: six storage
//! buffers here and the pool's two, the eight wgpu's default limits allow a
//! stage, #2417), and group 1, the renderer's one cluster pool.

use crate::gpu::layout_entries::{storage_entry, uniform_entry};
use crate::gpu::ops::CountedDevice as _;

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

/// A group 0 layout over `entries`, with the params at 0.
fn layout(
    device: &wgpu::Device,
    label: &str,
    entries: &[wgpu::BindGroupLayoutEntry],
) -> wgpu::BindGroupLayout {
    let mut all = vec![uniform_entry(0, wgpu::ShaderStages::COMPUTE)];
    all.extend_from_slice(entries);
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &all,
    })
}

/// The bindings the test's group 0 names, after the params: jobs, objects,
/// visible, state, the tally, the pyramid and the chunks.
pub(super) const TEST: [u32; 7] = [1, 2, 3, 4, 5, 7, 10];
/// The scan's: jobs, state and output args.
pub(super) const SCAN: [u32; 3] = [1, 4, 6];
/// The copy's: jobs, state, output indices and the chunks.
pub(super) const COPY: [u32; 4] = [1, 4, 9, 10];

/// The test's group 0.
pub(crate) fn test_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let pyramid = wgpu::BindGroupLayoutEntry {
        binding: 7,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    let entries = [
        storage(1, true),
        storage(2, true),
        storage(3, true),
        storage(4, false),
        storage(5, false),
        pyramid,
        storage(10, true),
    ];
    layout(device, "Cluster Test", &entries)
}

/// The scan's group 0.
pub(crate) fn scan_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let entries = [storage(1, true), storage(4, false), storage(6, false)];
    layout(device, "Cluster Scan", &entries)
}

/// The copy's group 0.
pub(crate) fn copy_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let entries = [
        storage(1, true),
        storage(4, false),
        storage(9, false),
        storage(10, true),
    ];
    layout(device, "Cluster Copy", &entries)
}

/// Group 1: the shared cluster table and index pool.
pub(crate) fn pool_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Cluster Pool"),
        entries: &[storage(0, true), storage(1, true)],
    })
}

/// A group 0 over `params` and `buffers` at `bindings`, the pyramid (7) a
/// texture where it is among them.
pub(super) fn stage_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    (params, bindings, buffers): (&wgpu::Buffer, &[u32], &[wgpu::Buffer]),
    pyramid: Option<&wgpu::TextureView>,
) -> wgpu::BindGroup {
    let mut buffers = buffers.iter();
    let mut entries = vec![wgpu::BindGroupEntry {
        binding: 0,
        resource: params.as_entire_binding(),
    }];
    entries.extend(bindings.iter().map(|&binding| {
        wgpu::BindGroupEntry {
            binding,
            resource: match (binding, pyramid) {
                (7, Some(view)) => wgpu::BindingResource::TextureView(view),
                _ => buffers
                    .next()
                    .expect("a buffer a binding")
                    .as_entire_binding(),
            },
        }
    }));
    device.counted_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Cluster Cull"),
        layout,
        entries: &entries,
    })
}
