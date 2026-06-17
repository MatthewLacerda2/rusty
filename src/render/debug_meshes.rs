use glam::Vec3;
use wgpu::util::DeviceExt;

use std::collections::HashSet;

use super::mesh::Vertex;
use super::{GpuMesh, MeshId, Renderer};
use crate::scene::Scene;

impl Renderer {
    /// Upload every active entity's mesh once per unique mesh-asset identity
    /// (#127): N entities sharing a primitive/asset collapse to a single buffer
    /// pair. `seen` dedups within the frame; resident-and-clean ids are skipped.
    /// Each mesh's dirty flag is cleared as it's visited, so a re-bake re-uploads
    /// the shared geometry exactly once.
    pub(super) fn upload_scene_meshes(&mut self, scene: &Scene) {
        let mut seen: HashSet<MeshId> = HashSet::new();
        let mut updates: Vec<(MeshId, Vec<Vertex>, Vec<u32>)> = Vec::new();
        for entity in scene.iter() {
            if !entity.active {
                continue;
            }
            if let Some(mesh) = &entity.mesh {
                let id = MeshId::from_mesh(mesh);
                let dirty = mesh.is_dirty.get();
                mesh.is_dirty.set(false);
                if (dirty || !self.gpu_meshes.contains_key(&id)) && seen.insert(id.clone()) {
                    updates.push((id, mesh.vertices.clone(), mesh.indices.clone()));
                }
            }
        }
        for (id, vertices, indices) in updates {
            self.update_gpu_mesh(id, &vertices, &indices);
        }
    }

    /// Uploads mesh data to the GPU, caching it under its mesh-asset identity
    /// (#127) so geometry shared by many entities is stored once. When a buffer
    /// for this `MeshId` is already resident and large enough, the data is written
    /// in place (`queue.write_buffer`) rather than reallocating a fresh buffer.
    pub fn update_gpu_mesh(&mut self, id: MeshId, vertices: &[Vertex], indices: &[u32]) {
        if vertices.is_empty() || indices.is_empty() {
            return;
        }

        let v_bytes: &[u8] = bytemuck::cast_slice(vertices);
        let i_bytes: &[u8] = bytemuck::cast_slice(indices);
        let num_indices = indices.len() as u32;

        // Re-use resident buffers when the new data still fits (the dirty-path
        // case: same geometry re-uploaded). Both buffers carry COPY_DST so they
        // can be written without reallocation.
        if let Some(existing) = self.gpu_meshes.get_mut(&id) {
            if existing.vertex_buffer.size() >= v_bytes.len() as u64
                && existing.index_buffer.size() >= i_bytes.len() as u64
            {
                existing.num_indices = num_indices;
                self.queue.write_buffer(&existing.vertex_buffer, 0, v_bytes);
                self.queue.write_buffer(&existing.index_buffer, 0, i_bytes);
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
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            });

        self.gpu_meshes.insert(
            id,
            GpuMesh {
                vertex_buffer,
                index_buffer,
                num_indices,
            },
        );
    }

    /// Pre-generates the static 3D line grid mesh for Editor visual feedback
    pub(super) fn generate_grid_mesh(&mut self) {
        let mut vertices = Vec::new();
        let spacing = 2.0;
        let extent = 30.0;

        let mut count = 0;
        let normal = Vec3::Y;
        let uv = [0.0, 0.0];

        // Draw horizontal and vertical lines in XZ plane
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

    /// Pre-generates 3D line-based arrows for X, Y, Z global translation axes
    #[allow(clippy::too_many_lines)]
    pub(super) fn generate_axis_arrows(&mut self) {
        let axis_length = 2.0;
        let arrow_head_length = 0.35;
        let arrow_head_width = 0.12;
        let uv = [0.0, 0.0];
        let normal = Vec3::Y;

        // X Axis (Red)
        let mut x_verts = Vec::new();
        let t_x = Vec3::new(axis_length, 0.0, 0.0);
        let s_x = Vec3::new(axis_length - arrow_head_length, 0.0, 0.0);
        x_verts.push(Vertex::new(Vec3::ZERO, normal, uv));
        x_verts.push(Vertex::new(t_x, normal, uv));
        // Arrowhead
        let a_x1 = s_x + Vec3::new(0.0, arrow_head_width, 0.0);
        let a_x2 = s_x - Vec3::new(0.0, arrow_head_width, 0.0);
        let a_x3 = s_x + Vec3::new(0.0, 0.0, arrow_head_width);
        let a_x4 = s_x - Vec3::new(0.0, 0.0, arrow_head_width);
        x_verts.push(Vertex::new(t_x, normal, uv));
        x_verts.push(Vertex::new(a_x1, normal, uv));
        x_verts.push(Vertex::new(t_x, normal, uv));
        x_verts.push(Vertex::new(a_x2, normal, uv));
        x_verts.push(Vertex::new(t_x, normal, uv));
        x_verts.push(Vertex::new(a_x3, normal, uv));
        x_verts.push(Vertex::new(t_x, normal, uv));
        x_verts.push(Vertex::new(a_x4, normal, uv));
        x_verts.push(Vertex::new(a_x1, normal, uv));
        x_verts.push(Vertex::new(a_x3, normal, uv));
        x_verts.push(Vertex::new(a_x3, normal, uv));
        x_verts.push(Vertex::new(a_x2, normal, uv));
        x_verts.push(Vertex::new(a_x2, normal, uv));
        x_verts.push(Vertex::new(a_x4, normal, uv));
        x_verts.push(Vertex::new(a_x4, normal, uv));
        x_verts.push(Vertex::new(a_x1, normal, uv));

        // Y Axis (Green)
        let mut y_verts = Vec::new();
        let t_y = Vec3::new(0.0, axis_length, 0.0);
        let s_y = Vec3::new(0.0, axis_length - arrow_head_length, 0.0);
        y_verts.push(Vertex::new(Vec3::ZERO, normal, uv));
        y_verts.push(Vertex::new(t_y, normal, uv));
        // Arrowhead
        let a_y1 = s_y + Vec3::new(arrow_head_width, 0.0, 0.0);
        let a_y2 = s_y - Vec3::new(arrow_head_width, 0.0, 0.0);
        let a_y3 = s_y + Vec3::new(0.0, 0.0, arrow_head_width);
        let a_y4 = s_y - Vec3::new(0.0, 0.0, arrow_head_width);
        y_verts.push(Vertex::new(t_y, normal, uv));
        y_verts.push(Vertex::new(a_y1, normal, uv));
        y_verts.push(Vertex::new(t_y, normal, uv));
        y_verts.push(Vertex::new(a_y2, normal, uv));
        y_verts.push(Vertex::new(t_y, normal, uv));
        y_verts.push(Vertex::new(a_y3, normal, uv));
        y_verts.push(Vertex::new(t_y, normal, uv));
        y_verts.push(Vertex::new(a_y4, normal, uv));
        y_verts.push(Vertex::new(a_y1, normal, uv));
        y_verts.push(Vertex::new(a_y3, normal, uv));
        y_verts.push(Vertex::new(a_y3, normal, uv));
        y_verts.push(Vertex::new(a_y2, normal, uv));
        y_verts.push(Vertex::new(a_y2, normal, uv));
        y_verts.push(Vertex::new(a_y4, normal, uv));
        y_verts.push(Vertex::new(a_y4, normal, uv));
        y_verts.push(Vertex::new(a_y1, normal, uv));

        // Z Axis (Blue)
        let mut z_verts = Vec::new();
        let t_z = Vec3::new(0.0, 0.0, axis_length);
        let s_z = Vec3::new(0.0, 0.0, axis_length - arrow_head_length);
        z_verts.push(Vertex::new(Vec3::ZERO, normal, uv));
        z_verts.push(Vertex::new(t_z, normal, uv));
        // Arrowhead
        let a_z1 = s_z + Vec3::new(arrow_head_width, 0.0, 0.0);
        let a_z2 = s_z - Vec3::new(arrow_head_width, 0.0, 0.0);
        let a_z3 = s_z + Vec3::new(0.0, arrow_head_width, 0.0);
        let a_z4 = s_z - Vec3::new(0.0, -arrow_head_width, 0.0);
        z_verts.push(Vertex::new(t_z, normal, uv));
        z_verts.push(Vertex::new(a_z1, normal, uv));
        z_verts.push(Vertex::new(t_z, normal, uv));
        z_verts.push(Vertex::new(a_z2, normal, uv));
        z_verts.push(Vertex::new(t_z, normal, uv));
        z_verts.push(Vertex::new(a_z3, normal, uv));
        z_verts.push(Vertex::new(t_z, normal, uv));
        z_verts.push(Vertex::new(a_z4, normal, uv));
        z_verts.push(Vertex::new(a_z1, normal, uv));
        z_verts.push(Vertex::new(a_z3, normal, uv));
        z_verts.push(Vertex::new(a_z3, normal, uv));
        z_verts.push(Vertex::new(a_z2, normal, uv));
        z_verts.push(Vertex::new(a_z2, normal, uv));
        z_verts.push(Vertex::new(a_z4, normal, uv));
        z_verts.push(Vertex::new(a_z4, normal, uv));
        z_verts.push(Vertex::new(a_z1, normal, uv));

        self.axis_x_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Axis X Buffer"),
                contents: bytemuck::cast_slice(&x_verts),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
        self.axis_y_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Axis Y Buffer"),
                contents: bytemuck::cast_slice(&y_verts),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
        self.axis_z_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Axis Z Buffer"),
                contents: bytemuck::cast_slice(&z_verts),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
        self.axis_count = x_verts.len() as u32;
    }
}
