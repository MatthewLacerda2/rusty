use wgpu::util::DeviceExt;

mod objects;
mod pack;

pub(crate) use pack::{
    draw_group, joint_array_bytes, push_palette, uniform_array_bytes, JointMatrix,
};

use super::grow_buffer::GrowBuffer;
use super::uniforms::ObjectData;
use crate::scene::{ObjectTable, SlotTable};
use crate::{DrawUniform, InstanceData, MaterialData};

/// Byte distance between per-draw uniforms: wgpu's default
/// `min_uniform_buffer_offset_alignment`, which the renderer requests.
pub(crate) const UNIFORM_STRIDE: usize = 256;
const _: () = assert!(std::mem::size_of::<DrawUniform>() <= UNIFORM_STRIDE);

pub(crate) const DRAW_GROUP_INSTANCES: usize = 2;

const IDENTITY_JOINT: [f32; 16] = glam::Mat4::IDENTITY.to_cols_array();

pub(crate) struct FrameUpload<'a> {
    pub uniforms: &'a [DrawUniform],
    pub joints: &'a [JointMatrix],
    pub instances: &'a [InstanceData],
    pub proxies: &'a [[f32; 4]],
    pub objects: &'a mut ObjectTable,
    pub materials: &'a mut SlotTable<MaterialData>,
}

pub(crate) struct DrawBuffers {
    uniforms: GrowBuffer,
    bones: GrowBuffer,
    instances: GrowBuffer,
    materials: GrowBuffer,
    proxies: GrowBuffer,
    objects: GrowBuffer,
    /// Bytes written since the last [`Self::take_written`] (`Debug.Stats`'
    /// `render_upload_bytes`).
    written: u64,
    bind_group: wgpu::BindGroup,
    default_bones: wgpu::Buffer,
    identity_instance: wgpu::Buffer,
}

impl DrawBuffers {
    pub(crate) fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        use wgpu::BufferUsages as U;
        let uniforms = GrowBuffer::new(device, "Frame Draw Uniforms", U::UNIFORM);
        let bones = GrowBuffer::new(device, "Frame Joint Matrices", U::STORAGE);
        let instances = GrowBuffer::new(device, "Frame Instances", U::STORAGE);
        let materials = GrowBuffer::new(device, "Material Records", U::STORAGE | U::COPY_SRC);
        let proxies = GrowBuffer::new(device, "Frame Proxy Volumes", U::STORAGE);
        let objects = GrowBuffer::new(device, "Object Records", U::STORAGE | U::COPY_SRC);
        let init = |label, contents: &[u8], usage| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
        };
        let default_bones = init(
            "Shared Default Bones",
            bytemuck::bytes_of(&IDENTITY_JOINT),
            U::STORAGE,
        );
        let identity_instance = init(
            "Identity Instance",
            bytemuck::bytes_of(&InstanceData::IDENTITY),
            U::STORAGE,
        );
        let bind_group = draw_group(
            device,
            layout,
            "Frame Draw Bind Group",
            [
                uniforms.buffer(),
                bones.buffer(),
                instances.buffer(),
                materials.buffer(),
                proxies.buffer(),
                objects.buffer(),
            ],
        );
        Self {
            uniforms,
            bones,
            instances,
            materials,
            proxies,
            objects,
            written: 0,
            bind_group,
            default_bones,
            identity_instance,
        }
    }

    pub(crate) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        frame: FrameUpload,
    ) {
        let FrameUpload {
            uniforms,
            joints,
            instances,
            proxies,
            objects,
            materials,
        } = frame;
        let uniform_bytes = uniform_array_bytes(uniforms);
        let bone_bytes = joint_array_bytes(joints);
        let gpu = (device, queue);
        let materials_grew =
            objects::sync_table(gpu, &mut self.materials, materials, &mut self.written);

        self.written += (uniform_bytes.len() + bone_bytes.len()) as u64;
        self.written += (std::mem::size_of_val(instances) + std::mem::size_of_val(proxies)) as u64;
        let grew = [
            self.uniforms.upload(device, queue, &uniform_bytes),
            self.bones.upload(device, queue, &bone_bytes),
            self.instances
                .upload(device, queue, bytemuck::cast_slice(instances)),
            materials_grew,
            self.proxies
                .upload(device, queue, bytemuck::cast_slice(proxies)),
            self.sync_objects(device, queue, objects),
        ];
        if grew.contains(&true) {
            self.rebind(device, layout);
        }
    }

    fn rebind(&mut self, device: &wgpu::Device, layout: &wgpu::BindGroupLayout) {
        let instances = self.instances.buffer();
        self.bind_group = self.group_over(device, layout, ("Frame Draw Bind Group", instances));
    }

    /// The bytes written since the last call.
    pub(crate) fn take_written(&mut self) -> u64 {
        std::mem::take(&mut self.written)
    }

    pub(crate) fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub(crate) fn offsets(slot: u32) -> [u32; 1] {
        [slot * UNIFORM_STRIDE as u32]
    }

    pub(crate) fn instances(&self) -> &wgpu::Buffer {
        self.instances.buffer()
    }

    pub(crate) fn group_with_instances(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        instances: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        self.group_over(device, layout, ("Culled Draw Bind Group", instances))
    }

    fn group_over(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        (label, instances): (&str, &wgpu::Buffer),
    ) -> wgpu::BindGroup {
        draw_group(device, layout, label, self.draw_group_buffers(instances))
    }

    pub(crate) fn draw_group_buffers<'a>(
        &'a self,
        instances: &'a wgpu::Buffer,
    ) -> [&'a wgpu::Buffer; 6] {
        let buffers = [
            self.uniforms.buffer(),
            self.bones.buffer(),
            instances,
            self.materials.buffer(),
            self.proxies.buffer(),
            self.objects.buffer(),
        ];
        debug_assert!(std::ptr::eq(buffers[DRAW_GROUP_INSTANCES], instances));
        buffers
    }

    pub(crate) fn default_bones(&self) -> &wgpu::Buffer {
        &self.default_bones
    }

    pub(crate) fn identity_instance(&self) -> &wgpu::Buffer {
        &self.identity_instance
    }

    /// The frame's proxy-volume array, which overlays bind and never read (their
    /// instance names none).
    pub(crate) fn proxies(&self) -> &wgpu::Buffer {
        self.proxies.buffer()
    }

    pub(crate) fn objects(&self) -> &wgpu::Buffer {
        self.objects.buffer()
    }
}
