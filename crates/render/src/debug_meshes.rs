use crate::gpu::ops::CountedQueue as _;
use glam::Vec3;
use wgpu::util::DeviceExt;

use std::collections::BTreeMap;

use super::{GpuMesh, MeshId, Renderer, SkinBounds};
use rusty_components::mesh::{MeshRevision, Vertex};
use rusty_scene::Scene;

const INDEX_USAGE: wgpu::BufferUsages = wgpu::BufferUsages::INDEX
    .union(wgpu::BufferUsages::STORAGE)
    .union(wgpu::BufferUsages::COPY_DST);

fn local_aabb(vertices: &[Vertex]) -> (Vec3, Vec3) {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for v in vertices {
        let p = Vec3::from_array(v.position);
        min = min.min(p);
        max = max.max(p);
    }
    if vertices.is_empty() {
        (Vec3::ZERO, Vec3::ZERO)
    } else {
        (min, max)
    }
}

pub(crate) fn has_geometry(vertices: &[Vertex], indices: &[u32]) -> bool {
    !vertices.is_empty() && !indices.is_empty()
}

pub(crate) struct UploadCandidate {
    pub id: MeshId,
    pub revision: MeshRevision,
    pub entity: u32,
    pub has_geometry: bool,
}

pub(crate) fn stale_uploads(
    candidates: impl IntoIterator<Item = UploadCandidate>,
    resident: impl Fn(&MeshId) -> Option<MeshRevision>,
) -> BTreeMap<MeshId, (MeshRevision, u32)> {
    let mut updates: BTreeMap<MeshId, (MeshRevision, u32)> = BTreeMap::new();
    for c in candidates {
        if !c.has_geometry || resident(&c.id).is_some_and(|r| r >= c.revision) {
            continue;
        }
        let newest = updates.entry(c.id).or_insert((c.revision, c.entity));
        if c.revision > newest.0 {
            *newest = (c.revision, c.entity);
        }
    }
    updates
}

impl Renderer {
    pub(crate) fn upload_meshes_of(&mut self, scene: &Scene, entities: &[u32]) {
        let candidates = entities.iter().filter_map(|&entity| {
            let mesh = scene.world.mesh(entity)?;
            Some(UploadCandidate {
                id: MeshId::from_mesh(&mesh),
                revision: mesh.revision,
                entity,
                has_geometry: has_geometry(&mesh.vertices, &mesh.indices),
            })
        });
        let resident = |id: &MeshId| self.gpu_meshes.get(id).map(|g| g.revision);
        for (id, (revision, entity)) in stale_uploads(candidates, resident) {
            let mesh = scene.world.mesh(entity).expect("collected above");
            self.upload_geometry(id, revision, &mesh);
        }
    }

    pub fn update_gpu_mesh(
        &mut self,
        id: MeshId,
        revision: MeshRevision,
        vertices: &[Vertex],
        indices: &[u32],
    ) {
        if !has_geometry(vertices, indices) {
            return;
        }

        let v_bytes: &[u8] = bytemuck::cast_slice(vertices);
        let i_bytes: &[u8] = bytemuck::cast_slice(indices);
        let local_aabb = local_aabb(vertices);
        let skin_bounds = SkinBounds::from_vertices(vertices);
        self.retained.meshes.uploaded(&id);
        self.uploads.add((v_bytes.len() + i_bytes.len()) as u64);

        let queue = &self.queue;
        if let Some(existing) = self.gpu_meshes.get_mut(&id) {
            if existing.vertex_buffer.size() >= v_bytes.len() as u64
                && existing.index_buffer.size() >= i_bytes.len() as u64
            {
                existing.num_indices = indices.len() as u32;
                existing.local_aabb = local_aabb;
                existing.skin_bounds = skin_bounds;
                existing.revision = revision;
                existing.clusters = None;
                queue.counted_write_buffer(&existing.vertex_buffer, 0, v_bytes);
                queue.counted_write_buffer(&existing.index_buffer, 0, i_bytes);
                return;
            }
        }

        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Mesh Vertices ({})", id.0)),
                contents: v_bytes,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });

        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Mesh Indices ({})", id.0)),
                contents: i_bytes,
                usage: INDEX_USAGE,
            });

        self.gpu_meshes.insert(
            id,
            GpuMesh {
                vertex_buffer,
                index_buffer,
                num_indices: indices.len() as u32,
                local_aabb,
                skin_bounds,
                revision,
                clusters: None,
            },
        );
    }

    pub(crate) fn generate_grid_mesh(&mut self) {
        let mut vertices = Vec::new();
        let spacing = 2.0;
        let extent = 30.0;

        let mut count = 0;
        let normal = Vec3::Y;
        let uv = [0.0, 0.0];

        let mut x = -extent;
        while x <= extent {
            vertices.push(Vertex::new(Vec3::new(x, 0.0, -extent), normal, uv));
            vertices.push(Vertex::new(Vec3::new(x, 0.0, extent), normal, uv));

            vertices.push(Vertex::new(Vec3::new(-extent, 0.0, x), normal, uv));
            vertices.push(Vertex::new(Vec3::new(extent, 0.0, x), normal, uv));

            x += spacing;
            count += 4;
        }

        self.grid_vertex_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Grid Mesh Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
        self.grid_count = count;
    }

    pub(crate) fn generate_axis_arrows(&mut self) {
        let axis_length = 2.0;
        let arrow_head_length = 0.35;
        let w = 0.12;

        let t_x = Vec3::new(axis_length, 0.0, 0.0);
        let s_x = Vec3::new(axis_length - arrow_head_length, 0.0, 0.0);
        let x_verts = axis_arrow_verts(
            t_x,
            [
                s_x + Vec3::new(0.0, w, 0.0),
                s_x - Vec3::new(0.0, w, 0.0),
                s_x + Vec3::new(0.0, 0.0, w),
                s_x - Vec3::new(0.0, 0.0, w),
            ],
        );

        let t_y = Vec3::new(0.0, axis_length, 0.0);
        let s_y = Vec3::new(0.0, axis_length - arrow_head_length, 0.0);
        let y_verts = axis_arrow_verts(
            t_y,
            [
                s_y + Vec3::new(w, 0.0, 0.0),
                s_y - Vec3::new(w, 0.0, 0.0),
                s_y + Vec3::new(0.0, 0.0, w),
                s_y - Vec3::new(0.0, 0.0, w),
            ],
        );

        let t_z = Vec3::new(0.0, 0.0, axis_length);
        let s_z = Vec3::new(0.0, 0.0, axis_length - arrow_head_length);
        let z_verts = axis_arrow_verts(
            t_z,
            [
                s_z + Vec3::new(w, 0.0, 0.0),
                s_z - Vec3::new(w, 0.0, 0.0),
                s_z + Vec3::new(0.0, w, 0.0),
                s_z - Vec3::new(0.0, -w, 0.0),
            ],
        );

        self.axis_x_buffer = Some(self.vertex_buffer("Axis X Buffer", &x_verts));
        self.axis_y_buffer = Some(self.vertex_buffer("Axis Y Buffer", &y_verts));
        self.axis_z_buffer = Some(self.vertex_buffer("Axis Z Buffer", &z_verts));
        self.axis_count = x_verts.len() as u32;
    }

    fn vertex_buffer(&self, label: &str, verts: &[Vertex]) -> wgpu::Buffer {
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(verts),
                usage: wgpu::BufferUsages::VERTEX,
            })
    }
}

fn axis_arrow_verts(tip: Vec3, head: [Vec3; 4]) -> Vec<Vertex> {
    let uv = [0.0, 0.0];
    let normal = Vec3::Y;
    let [a1, a2, a3, a4] = head;

    let mut verts = Vec::new();
    verts.push(Vertex::new(Vec3::ZERO, normal, uv));
    verts.push(Vertex::new(tip, normal, uv));
    for a in [a1, a2, a3, a4] {
        verts.push(Vertex::new(tip, normal, uv));
        verts.push(Vertex::new(a, normal, uv));
    }
    for (p, q) in [(a1, a3), (a3, a2), (a2, a4), (a4, a1)] {
        verts.push(Vertex::new(p, normal, uv));
        verts.push(Vertex::new(q, normal, uv));
    }
    verts
}

#[cfg(test)]
#[path = "debug_meshes_tests.rs"]
mod tests;
