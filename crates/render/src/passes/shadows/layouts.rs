use crate::gpu::groups::Groups;
use crate::gpu::layout_entries::{dynamic_uniform_entry, storage_entry, uniform_entry};
use crate::gpu::ops::CountedDevice as _;

// These mirror `forward/shadow_caster.wgsl`'s bindings; the 64 bytes are its `ShadowLight` matrix.
const LIGHT_BINDING: u32 = 6;
/// The forward `camera` slot, holding only the game time for moving cuts.
const TIME_BINDING: u32 = 0;
pub(super) const CASTERS_BINDING: u32 = 3;
pub(super) const JOINTS_BINDING: u32 = 1;
pub(super) const DRAW_BINDING: u32 = 0;
pub(super) const MATERIALS_BINDING: u32 = 4;

pub(super) fn global_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Shadow Global Layout"),
        entries: &light_entries(),
    })
}

pub(crate) fn light_entries() -> [wgpu::BindGroupLayoutEntry; 2] {
    [
        dynamic_uniform_entry(LIGHT_BINDING, wgpu::ShaderStages::VERTEX, 64),
        uniform_entry(TIME_BINDING, wgpu::ShaderStages::VERTEX_FRAGMENT),
    ]
}

pub(crate) fn caster_entries() -> [wgpu::BindGroupLayoutEntry; 4] {
    let both = wgpu::ShaderStages::VERTEX_FRAGMENT;
    let draw_size = std::mem::size_of::<crate::DrawUniform>() as u64;
    [
        storage_entry(CASTERS_BINDING, both),
        storage_entry(JOINTS_BINDING, wgpu::ShaderStages::VERTEX),
        dynamic_uniform_entry(DRAW_BINDING, both, draw_size),
        storage_entry(MATERIALS_BINDING, both),
    ]
}

pub(super) fn global_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    light_space: &wgpu::Buffer,
    time_buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.counted_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Shadow Global Bind Group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: LIGHT_BINDING,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: light_space,
                    offset: 0,
                    size: wgpu::BufferSize::new(64),
                }),
            },
            wgpu::BindGroupEntry {
                binding: TIME_BINDING,
                resource: time_buffer.as_entire_binding(),
            },
        ],
    })
}

pub(crate) fn clip_layout(
    device: &wgpu::Device,
    [global, casters, material]: [&wgpu::BindGroupLayout; 3],
) -> wgpu::PipelineLayout {
    let groups = Groups {
        global,
        scene: None,
        material: Some(material),
        draw: casters,
    };
    groups.layout(device, "Shadow Clip Pipeline Layout")
}
