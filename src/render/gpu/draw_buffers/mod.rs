//! src/render/gpu/draw_buffers/ — the forward pass's instancing buffers (#470), and
//! the resources every group-1 bind group is built from.
//!
//! This replaces the per-entity pool of #210 (one uniform buffer, bone buffer and two
//! bind groups kept alive per entity). A camera's solids are four packed arrays —
//! per-draw uniforms at a 256-byte stride, joint matrices, instances and the Light
//! Probe Proxy Volumes the instances index (#1672) — and the two kept tables below
//! (objects, and the material records the uniforms index, #1616), behind **one** bind
//! group whose dynamic offset picks a batch's
//! uniform. The per-camera arrays are written with a `write_buffer` each. Nothing is
//! keyed by entity, so two scenes rendered in one frame (#355) cannot evict each
//! other: each render simply rewrites the arrays before its own submit.
//!
//! **The object array** (#1706) is not packed per camera: every kept
//! object's [`ObjectData`] (world matrix, probe SH, lightmap) at its slot of the
//! renderer's `scene::ObjectTable`, written where it changed (`objects.rs`). A
//! camera's instance is 16 bytes naming its object's slot, so the per-camera upload
//! no longer grows with what the objects hold.
//!
//! **The material array** is held the same way (#1706): every kept material record
//! (one per scene, library material and variant, `scene::MaterialRecords`) at its slot,
//! written where it changed. A draw's uniform names the slot (its value class's
//! least, so equal records batch), so a colour edit uploads that one record and no
//! camera re-packs records.
//!
//! The joint matrices are one storage array (#455), not a fixed 64-matrix uniform: a
//! skinned draw's palette is its run of the array, as long as its skin, and the draw's
//! uniform carries where that run starts (`bone_base`). Element 0 is one identity
//! matrix, which every non-skinned draw and overlay points at. A view writing motion
//! vectors appends each instance's last-frame world matrix and palette there too
//! (`motion::track`).

use wgpu::util::DeviceExt;

mod objects;
mod pack;

pub(crate) use pack::{group1, joint_array_bytes, push_palette, uniform_array_bytes, JointMatrix};

use super::grow_buffer::GrowBuffer;
use super::uniforms::ObjectData;
use crate::render::scene::{ObjectTable, SlotTable};
use crate::render::{DrawUniform, InstanceData, MaterialData};

/// Byte distance between per-draw uniforms: wgpu's default
/// `min_uniform_buffer_offset_alignment`, which the renderer requests.
pub(crate) const UNIFORM_STRIDE: usize = 256;
const _: () = assert!(std::mem::size_of::<DrawUniform>() <= UNIFORM_STRIDE);

/// The shared identity joint: element 0 of the frame's joint array, and the whole of
/// the array overlays bind.
const IDENTITY_JOINT: [f32; 16] = glam::Mat4::IDENTITY.to_cols_array();

/// One camera's draw data: `uniforms` in slot order, the skinned draws' `joints`
/// back to back (a draw whose `bone_base` is `n` starts at `joints[n - 1]`; element 0
/// of the uploaded array is the identity), the packed `instances`, and the
/// `proxies` the instances index (`draw::proxy_volume`), and the slot tables the
/// uniforms and instances name (materials and objects), whose changed slots and
/// transient ones are written with them.
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
    /// Every kept object's record, by slot (`objects.rs`).
    objects: GrowBuffer,
    /// Bytes written since the last [`Self::take_written`] (`Debug.Stats`'
    /// `render_upload_bytes`).
    written: u64,
    bind_group: wgpu::BindGroup,
    /// The identity joint alone, for overlays (they bind their own uniform).
    default_bones: wgpu::Buffer,
    /// A one-element instance array holding [`InstanceData::IDENTITY`], for overlays.
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
        // Copyable, so a test can read back that it mirrors the table.
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
        let bind_group = group1(
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

    /// Write one camera's draw data, and the object and material slots that changed
    /// since the last upload. Rebuilds the bind group only when a buffer had to grow.
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

    /// Rebuild the bind group over the buffers as they are now (one grew).
    fn rebind(&mut self, device: &wgpu::Device, layout: &wgpu::BindGroupLayout) {
        let instances = self.instances.buffer();
        self.bind_group = self.group_over(device, layout, ("Frame Draw Bind Group", instances));
    }

    /// The bytes written since the last call.
    pub(crate) fn take_written(&mut self) -> u64 {
        std::mem::take(&mut self.written)
    }

    /// The one group-1 bind group every solid draw binds, at [`Self::offsets`].
    pub(crate) fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// The dynamic offset selecting uniform `slot`.
    pub(crate) fn offsets(slot: u32) -> [u32; 1] {
        [slot * UNIFORM_STRIDE as u32]
    }

    /// The camera's packed instances, which GPU occlusion culling reads (#1963).
    pub(crate) fn instances(&self) -> &wgpu::Buffer {
        self.instances.buffer()
    }

    /// A group 1 over these buffers with `instances` in place of the camera's own:
    /// the instances GPU occlusion culling kept (#1963), at their batches' runs.
    pub(crate) fn group_with_instances(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        instances: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        self.group_over(device, layout, ("Culled Draw Bind Group", instances))
    }

    /// The group 1 over these buffers and `instances`, labelled.
    fn group_over(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        (label, instances): (&str, &wgpu::Buffer),
    ) -> wgpu::BindGroup {
        let buffers = [
            self.uniforms.buffer(),
            self.bones.buffer(),
            instances,
            self.materials.buffer(),
            self.proxies.buffer(),
            self.objects.buffer(),
        ];
        group1(device, layout, label, buffers)
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

    /// The object array, which overlays bind for its identity at slot 0.
    pub(crate) fn objects(&self) -> &wgpu::Buffer {
        self.objects.buffer()
    }
}
