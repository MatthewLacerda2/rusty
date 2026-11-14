//! A sweep's caster data on the GPU: the per-caster array `shader.wgsl`'s shadow stage
//! indexes by instance, the joint array its skinned casters' palettes live in (#599),
//! and the material records its clipped draws read through `material()` (#1616), as
//! Unity's ShadowCaster pass reads the material: a surface variant's vertex blocks
//! and cut run in `vs_shadow` / `fs_shadow_cut` against the record their forward draw
//! reads. Each record has a per-draw uniform of its own (`draw.material` its index,
//! the rest unread by the shadow stage), picked by dynamic offset.

use crate::gpu::draw_buffers::{
    joint_array_bytes, uniform_array_bytes, JointMatrix, UNIFORM_STRIDE,
};
use crate::gpu::grow_buffer::GrowBuffer;
use crate::gpu::uniforms::MappingUniform;
use crate::passes::shadows::clips::{Clip, Cutout};
use crate::passes::shadows::setup::{
    CASTERS_BINDING, DRAW_BINDING, JOINTS_BINDING, MATERIALS_BINDING,
};
use crate::scene::SlotTable;
use crate::{DrawUniform, MaterialData};

/// One caster as `shader.wgsl`'s `Caster` reads it: world matrix, where its palette
/// starts in the joint array (0 — the identity — for an unskinned mesh), and its
/// cutout alpha test (#648; a cutoff of 0 clips nothing) at the texels its material's
/// mapping terms (`terms`) read (#1594). The mapping rides the instance, as the cutoff does, so
/// casters that differ only in it still share one draw and each clips by its own.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(in crate::passes::shadows) struct CasterData {
    pub model: [f32; 16],
    pub bone_base: u32,
    pub alpha_cutoff: f32,
    pub use_texture: u32,
    _pad: u32,
    pub terms: MappingUniform,
}

impl CasterData {
    /// A caster whose shadow is not alpha-tested; [`Self::cutout`] adds the test.
    pub fn new(model: glam::Mat4, bone_base: u32) -> Self {
        Self {
            model: model.to_cols_array(),
            bone_base,
            alpha_cutoff: 0.0,
            use_texture: 0,
            _pad: 0,
            terms: MappingUniform::default(),
        }
    }

    /// This caster, its shadow clipped where its albedo's alpha (when textured) is
    /// below the cutoff — a Cutout material's test (#242, #648), read through its
    /// mapping (#1594).
    pub fn cutout(self, cut: Cutout) -> Self {
        Self {
            alpha_cutoff: cut.cutoff,
            use_texture: u32::from(cut.textured),
            terms: cut.mapping,
            ..self
        }
    }
}

/// A sweep's distinct material records: every variant clip's record, by its slot of
/// the material table (#1706), and slot 0, the flat one that plain draws and a
/// cutout's standard clip (which reads its test per caster) bind and never read.
/// Each is read from the table as it is now, so a record-only edit's cut follows
/// with no caster re-derived.
pub(in crate::passes::shadows) struct CasterRecords {
    /// The slots read, sorted, each once.
    slots: Vec<u32>,
    records: Vec<MaterialData>,
}

impl CasterRecords {
    /// The records `clips` read, beside the flat one, as `table` holds them.
    pub(in crate::passes::shadows) fn new<'a>(
        clips: impl IntoIterator<Item = Option<&'a Clip>>,
        table: &SlotTable<MaterialData>,
    ) -> Self {
        let read = clips.into_iter().flatten().filter_map(|c| c.record);
        let mut slots: Vec<u32> = read.chain([FLAT]).collect();
        slots.sort_unstable();
        slots.dedup();
        let records = slots.iter().map(|&s| *table.get(s)).collect();
        Self { slots, records }
    }

    /// The index of the record `clip` reads (the flat one when it reads none).
    pub(in crate::passes::shadows) fn index(&self, clip: Option<&Clip>) -> u32 {
        let slot = clip.and_then(|c| c.record).unwrap_or(FLAT);
        let found = self.slots.binary_search(&slot);
        found.expect("built from every clip its draws name") as u32
    }

    /// One per-draw uniform per record, naming it.
    fn draws(&self) -> Vec<DrawUniform> {
        let draw = |material| DrawUniform {
            model_matrix: glam::Mat4::IDENTITY.to_cols_array(),
            bone_base: 0,
            material,
            _pad: [0; 2],
        };
        (0..self.slots.len() as u32).map(draw).collect()
    }
}

/// The material table's slot 0: the flat record a draw that reads none binds.
const FLAT: u32 = 0;

/// The caster, joint and record arrays and the bind group that exposes them to the
/// shadow stage.
pub(in crate::passes::shadows) struct CasterBuffer {
    casters: GrowBuffer,
    joints: GrowBuffer,
    draws: GrowBuffer,
    materials: GrowBuffer,
    pub(super) bind_group: wgpu::BindGroup,
}

/// The sweep's packed data, as [`CasterBuffer::upload`] takes it.
pub(in crate::passes::shadows) struct CasterUpload<'a> {
    pub casters: &'a [CasterData],
    pub joints: &'a [JointMatrix],
    pub records: &'a CasterRecords,
}

impl CasterBuffer {
    pub fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let storage = wgpu::BufferUsages::STORAGE;
        let buffers = [
            GrowBuffer::new(device, "Shadow Casters", storage),
            GrowBuffer::new(device, "Shadow Caster Joints", storage),
            GrowBuffer::new(device, "Shadow Caster Draws", wgpu::BufferUsages::UNIFORM),
            GrowBuffer::new(device, "Shadow Caster Materials", storage),
        ];
        let bind_group = caster_group(device, layout, buffers.each_ref());
        let [casters, joints, draws, materials] = buffers;
        Self {
            casters,
            joints,
            draws,
            materials,
            bind_group,
        }
    }

    /// Upload the sweep's data, rebuilding the bind group if a buffer had to grow.
    pub(super) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        sweep: CasterUpload,
    ) {
        let records = &sweep.records.records;
        let grew = [
            self.casters
                .upload(device, queue, bytemuck::cast_slice(sweep.casters)),
            self.joints
                .upload(device, queue, &joint_array_bytes(sweep.joints)),
            self.draws
                .upload(device, queue, &uniform_array_bytes(&sweep.records.draws())),
            self.materials
                .upload(device, queue, bytemuck::cast_slice(records)),
        ];
        if grew.contains(&true) {
            let buffers = [&self.casters, &self.joints, &self.draws, &self.materials];
            self.bind_group = caster_group(device, layout, buffers);
        }
    }

    /// The dynamic offset selecting record `record`'s per-draw uniform.
    pub(super) fn offsets(record: u32) -> [u32; 1] {
        [record * UNIFORM_STRIDE as u32]
    }
}

fn caster_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    [casters, joints, draws, materials]: [&GrowBuffer; 4],
) -> wgpu::BindGroup {
    fn whole(binding: u32, buffer: &GrowBuffer) -> wgpu::BindGroupEntry<'_> {
        wgpu::BindGroupEntry {
            binding,
            resource: buffer.buffer().as_entire_binding(),
        }
    }
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Shadow Caster Bind Group"),
        layout,
        entries: &[
            whole(CASTERS_BINDING, casters),
            whole(JOINTS_BINDING, joints),
            whole(MATERIALS_BINDING, materials),
            // One uniform's width, so the dynamic offset slides across the array.
            wgpu::BindGroupEntry {
                binding: DRAW_BINDING,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: draws.buffer(),
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<DrawUniform>() as u64),
                }),
            },
        ],
    })
}
