use wgpu::util::DeviceExt;

use super::build::Node;
use super::top::Packed;
use crate::gpu::layout_entries::{storage_entry, uniform_entry};
use crate::gpu::ops::{CountedDevice as _, CountedQueue as _};

const STAGES: wgpu::ShaderStages = wgpu::ShaderStages::COMPUTE.union(wgpu::ShaderStages::FRAGMENT);
#[cfg(not(test))]
const HEADER_USAGE: wgpu::BufferUsages = wgpu::BufferUsages::UNIFORM;
#[cfg(test)]
const HEADER_USAGE: wgpu::BufferUsages =
    wgpu::BufferUsages::UNIFORM.union(wgpu::BufferUsages::COPY_SRC);
const NODE: u64 = std::mem::size_of::<Node>() as u64;

/// `rt::trace`'s group: the header, then the nodes, triangles, UVs, instances and faces.
pub(crate) fn layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let mut entries = vec![uniform_entry(0, STAGES)];
    entries.extend((1..6).map(|b| storage_entry(b, STAGES)));
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Ray Tree"),
        entries: &entries,
    })
}

pub(super) struct GpuTree {
    header: wgpu::Buffer,
    nodes: wgpu::Buffer,
    tris: wgpu::Buffer,
    uvs: wgpu::Buffer,
    instances: wgpu::Buffer,
    faces: wgpu::Buffer,
    pub(super) group: wgpu::BindGroup,
}

type Geometry = (wgpu::Buffer, wgpu::Buffer, wgpu::Buffer);

/// The first buffer past the device's limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Refused {
    pub buffer: &'static str,
    pub bytes: u64,
    pub limit: u64,
}

#[cfg(test)]
thread_local! {
    pub(super) static LIMIT: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

/// The lesser of the device's binding and buffer limits.
fn limit(device: &wgpu::Device) -> u64 {
    #[cfg(test)]
    if let Some(limit) = LIMIT.get() {
        return limit;
    }
    let limits = device.limits();
    limits
        .max_storage_buffer_binding_size
        .min(limits.max_buffer_size)
}

fn fits(device: &wgpu::Device, sizes: &[(&'static str, u64)]) -> Result<(), Refused> {
    let limit = limit(device);
    match sizes.iter().find(|(_, bytes)| *bytes > limit) {
        Some(&(buffer, bytes)) => Err(Refused {
            buffer,
            bytes,
            limit,
        }),
        None => Ok(()),
    }
}

fn bytes<T>(items: usize) -> u64 {
    (items.max(1) * std::mem::size_of::<T>()) as u64
}

/// Made again on every upload.
fn top_sizes(p: &Packed) -> [(&'static str, u64); 2] {
    [
        (
            "instances",
            bytes::<super::top::Instance>(p.instances.len()),
        ),
        ("faces", bytes::<u32>(p.faces.len())),
    ]
}
type Gpu<'a> = (&'a wgpu::Device, &'a wgpu::Queue, &'a wgpu::BindGroupLayout);

/// A buffer holding `items`, or one zeroed item when there are none.
fn buffer<T: bytemuck::Pod>(device: &wgpu::Device, label: &str, items: &[T]) -> wgpu::Buffer {
    let one = [T::zeroed()];
    let items = if items.is_empty() { &one[..] } else { items };
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(items),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
    })
}

impl GpuTree {
    /// All of `p`, with room for the top to grow; refused whole past the limit.
    pub(super) fn whole(gpu: Gpu, p: &Packed) -> Result<Self, Refused> {
        let device = gpu.0;
        let room = p.header.instances as usize * 2 + 64;
        let mut sizes = vec![
            ("nodes", bytes::<Node>(p.nodes.len() + room)),
            ("triangles", bytes::<super::mesh::Tri>(p.tris.len())),
            ("UVs", bytes::<super::mesh::TriUv>(p.uvs.len())),
        ];
        sizes.extend(top_sizes(p));
        fits(device, &sizes)?;
        Ok(Self::upload(gpu, p, room))
    }

    /// Bound in place of a refused tree.
    pub(super) fn empty(gpu: Gpu) -> Self {
        Self::upload(gpu, &Packed::empty(), 0)
    }

    fn upload(gpu: Gpu, p: &Packed, room: usize) -> Self {
        let device = gpu.0;
        let nodes: Vec<Node> = p
            .nodes
            .iter()
            .copied()
            .chain((0..room).map(|_| Node::default()))
            .collect();
        let nodes = buffer(device, "Ray Tree Nodes", &nodes);
        let tris = buffer(device, "Ray Tree Triangles", &p.tris);
        let uvs = buffer(device, "Ray Tree UVs", &p.uvs);
        Self::with(gpu, p, (nodes, tris, uvs))
    }

    /// Only the top, after the first `geometry` nodes; `None` without room.
    pub(super) fn top(self, gpu: Gpu, top: &Packed, geometry: u32) -> Option<Self> {
        let end = u64::from(geometry) + top.nodes.len() as u64;
        if self.nodes.size() < NODE * end || fits(gpu.0, &top_sizes(top)).is_err() {
            return None;
        }
        let at = NODE * u64::from(geometry);
        gpu.1
            .counted_write_buffer(&self.nodes, at, bytemuck::cast_slice(&top.nodes));
        Some(Self::with(gpu, top, (self.nodes, self.tris, self.uvs)))
    }

    fn with((device, _, layout): Gpu, p: &Packed, (nodes, tris, uvs): Geometry) -> Self {
        let header = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Ray Tree Header"),
            contents: bytemuck::bytes_of(&p.header),
            usage: HEADER_USAGE,
        });
        let instances = buffer(device, "Ray Tree Instances", &p.instances);
        let faces = buffer(device, "Ray Tree Faces", &p.faces);
        let all = [&header, &nodes, &tris, &uvs, &instances, &faces];
        let entries: Vec<wgpu::BindGroupEntry> = (0..)
            .zip(all)
            .map(|(binding, b)| wgpu::BindGroupEntry {
                binding,
                resource: b.as_entire_binding(),
            })
            .collect();
        let group = device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Ray Tree"),
            layout,
            entries: &entries,
        });
        Self {
            header,
            nodes,
            tris,
            uvs,
            instances,
            faces,
            group,
        }
    }

    #[cfg(test)]
    pub(super) fn buffers(&self) -> [&wgpu::Buffer; 6] {
        [
            &self.header,
            &self.nodes,
            &self.tris,
            &self.uvs,
            &self.instances,
            &self.faces,
        ]
    }

    pub(super) fn bytes(&self) -> u64 {
        let all = [
            &self.header,
            &self.nodes,
            &self.tris,
            &self.uvs,
            &self.instances,
            &self.faces,
        ];
        all.iter().map(|b| b.size()).sum()
    }
}
