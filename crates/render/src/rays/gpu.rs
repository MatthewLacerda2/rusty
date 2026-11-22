use wgpu::util::DeviceExt;

use super::build::Node;
use super::top::Packed;
use crate::gpu::layout_entries::{storage_entry, uniform_entry};
use crate::gpu::ops::{CountedDevice as _, CountedQueue as _};

const STAGES: wgpu::ShaderStages = wgpu::ShaderStages::COMPUTE.union(wgpu::ShaderStages::FRAGMENT);
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
    /// Uploads `p`; with `kept` (whose meshes are `p`'s first `geometry` nodes), writes
    /// only the top tree and its instances.
    pub(super) fn upload(
        (device, queue, layout): Gpu,
        p: &Packed,
        geometry: u32,
        kept: Option<Self>,
    ) -> Self {
        let header = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Ray Tree Header"),
            contents: bytemuck::bytes_of(&p.header),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let instances = buffer(device, "Ray Tree Instances", &p.instances);
        let faces = buffer(device, "Ray Tree Faces", &p.faces);
        let (nodes, tris, uvs) = match kept {
            Some(k) if k.nodes.size() >= NODE * p.nodes.len() as u64 => {
                let top = &p.nodes[geometry as usize..];
                let at = NODE * u64::from(geometry);
                queue.counted_write_buffer(&k.nodes, at, bytemuck::cast_slice(top));
                (k.nodes, k.tris, k.uvs)
            }
            Some(k) => (buffer(device, "Ray Tree Nodes", &p.nodes), k.tris, k.uvs),
            None => (
                buffer(device, "Ray Tree Nodes", &p.nodes),
                buffer(device, "Ray Tree Triangles", &p.tris),
                buffer(device, "Ray Tree UVs", &p.uvs),
            ),
        };
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
