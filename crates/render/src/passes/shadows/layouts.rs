//! The shadow pass's bind-group layouts: the light group, the caster group, and
//! the pipeline layouts that place them ([`Groups`]).

use crate::gpu::groups::Groups;
use crate::gpu::layout_entries::{dynamic_uniform_entry, storage_entry, uniform_entry};
use crate::gpu::ops::CountedDevice as _;

/// Where `shader.wgsl`'s shadow stage reads the cascade's matrix and the game time
/// (the global group), its casters, their joints and the material records with the
/// per-draw uniform naming one (the per-draw group, the last two the forward pass's
/// own) — beside the forward pass's bindings, sharing `camera` and `bones` (#648).
const LIGHT_BINDING: u32 = 6;
const TIME_BINDING: u32 = 0;
pub(super) const CASTERS_BINDING: u32 = 3;
pub(super) const JOINTS_BINDING: u32 = 1;
pub(super) const DRAW_BINDING: u32 = 0;
pub(super) const MATERIALS_BINDING: u32 = 4;

/// The shadow stage's group 0: the cascade's light-space matrix (dynamic offset,
/// one per cascade) and the game time.
pub(super) fn global_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Shadow Global Layout"),
        entries: &light_entries(),
    })
}

/// [`global_layout`]'s entries.
pub(crate) fn light_entries() -> [wgpu::BindGroupLayoutEntry; 2] {
    [
        dynamic_uniform_entry(LIGHT_BINDING, wgpu::ShaderStages::VERTEX, 64),
        // The forward `camera` slot, holding only the game time: a cutting
        // variant's UV chain reads `camera.time` (#648), and `vs_shadow` moves
        // the caster with a variant's vertex blocks at it (#1524).
        uniform_entry(TIME_BINDING, wgpu::ShaderStages::VERTEX_FRAGMENT),
    ]
}

/// The caster group's entries: the casters, their joints, the per-draw uniform and
/// the material records.
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

/// A shadow stage's group 0 over `layout`: one light matrix of `light_space` (picked
/// by dynamic offset) and the game time — the cascades' and the atlas's (#468).
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

/// The clipping shadow pipelines' layout: light, material, casters (#648). A
/// shadow caster's groups by role ([`Groups`]): the light at the forward pass's
/// global slot, its material only when it clips, and the casters at the per-draw
/// slot, last; the scene slot is a gap.
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
