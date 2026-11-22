use crate::gpu::draw_buffers::{
    joint_array_bytes, uniform_array_bytes, JointMatrix, UNIFORM_STRIDE,
};
use crate::gpu::grow_buffer::GrowBuffer;
use crate::gpu::ops::CountedDevice as _;
use crate::gpu::uniforms::MappingUniform;
use crate::passes::shadows::clips::{Clip, Cutout};
use crate::passes::shadows::layouts::{
    CASTERS_BINDING, DRAW_BINDING, JOINTS_BINDING, MATERIALS_BINDING,
};
use crate::scene::SlotTable;
use crate::{DrawUniform, MaterialData};

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(in crate::passes::shadows) struct CasterData {
    pub model: [f32; 16],
    /// Where its joint palette starts in `bones`; 0, the shared identity joint, when unskinned.
    pub bone_base: u32,
    /// 0 clips nothing.
    pub alpha_cutoff: f32,
    pub use_texture: u32,
    _pad: u32,
    pub terms: MappingUniform,
}

impl CasterData {
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

    pub fn cutout(self, cut: Cutout) -> Self {
        Self {
            alpha_cutoff: cut.cutoff,
            use_texture: u32::from(cut.textured),
            terms: cut.mapping,
            ..self
        }
    }
}

/// Read from the table each sweep, so a record-only edit's cut follows.
pub(in crate::passes::shadows) struct CasterRecords {
    slots: Vec<u32>,
    records: Vec<MaterialData>,
}

impl CasterRecords {
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

    pub(in crate::passes::shadows) fn index(&self, clip: Option<&Clip>) -> u32 {
        let slot = clip.and_then(|c| c.record).unwrap_or(FLAT);
        let found = self.slots.binary_search(&slot);
        found.expect("built from every clip its draws name") as u32
    }

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

/// The material table's slot 0: plain draws and a cutout's standard clip bind it, never read it.
const FLAT: u32 = 0;

/// One per sweep: every sweep records before one submit, so a shared buffer's later upload
/// would overwrite earlier casters.
pub(in crate::passes::shadows) struct CasterBuffer {
    casters: GrowBuffer,
    joints: GrowBuffer,
    draws: GrowBuffer,
    materials: GrowBuffer,
    pub(super) bind_group: wgpu::BindGroup,
}

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
    device.counted_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Shadow Caster Bind Group"),
        layout,
        entries: &[
            whole(CASTERS_BINDING, casters),
            whole(JOINTS_BINDING, joints),
            whole(MATERIALS_BINDING, materials),
            wgpu::BindGroupEntry {
                binding: DRAW_BINDING,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: draws.buffer(),
                    offset: 0,
                    // One uniform's width, so the dynamic offset slides across the array.
                    size: wgpu::BufferSize::new(std::mem::size_of::<DrawUniform>() as u64),
                }),
            },
        ],
    })
}
