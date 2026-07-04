use glam::Vec3;
use wgpu::util::DeviceExt;

use std::collections::HashSet;

use super::{GpuMesh, MeshId, Renderer};
use crate::render::gpu::mesh::Vertex;
use crate::scene::Scene;

/// The local-space AABB `(min, max)` over a mesh's vertices. Callers upload only
/// non-empty geometry, so the fold always sees at least one vertex; an empty slice
/// degenerates to a zero-extent box at the origin (never rendered).
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

impl Renderer {
    /// Upload every active entity's mesh once per unique mesh-asset identity
    /// (#127): N entities sharing a primitive/asset collapse to a single buffer
    /// pair. `seen` dedups within the frame; resident-and-clean ids are skipped.
    /// Each mesh's dirty flag is cleared as it's visited, so a re-bake re-uploads
    /// the shared geometry exactly once.
    pub(crate) fn upload_scene_meshes(&mut self, scene: &Scene) {
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
        // Local-space AABB computed once here, at upload, so frustum culling never walks
        // the vertices per frame (#330). Re-derived on every re-upload so an edited mesh's
        // bounds stay correct.
        let local_aabb = local_aabb(vertices);

        // Re-use resident buffers when the new data still fits (the dirty-path
        // case: same geometry re-uploaded). Both buffers carry COPY_DST so they
        // can be written without reallocation.
        if let Some(existing) = self.gpu_meshes.get_mut(&id) {
            if existing.vertex_buffer.size() >= v_bytes.len() as u64
                && existing.index_buffer.size() >= i_bytes.len() as u64
            {
                existing.num_indices = num_indices;
                existing.local_aabb = local_aabb;
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
                local_aabb,
            },
        );
    }

    /// Pre-generates the static 3D line grid mesh for Editor visual feedback
    pub(crate) fn generate_grid_mesh(&mut self) {
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
    pub(crate) fn generate_axis_arrows(&mut self) {
        let axis_length = 2.0;
        let arrow_head_length = 0.35;
        let w = 0.12; // arrow head half-width

        // X Axis (Red): tip on +X, arrowhead spread on ±Y and ±Z.
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

        // Y Axis (Green): tip on +Y, arrowhead spread on ±X and ±Z.
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

        // Z Axis (Blue): tip on +Z, arrowhead spread on ±X and ±Y. (Preserves the
        // original quirk where the third and fourth head points coincide.)
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

    /// Create a VERTEX buffer initialised from `verts`.
    fn vertex_buffer(&self, label: &str, verts: &[Vertex]) -> wgpu::Buffer {
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(verts),
                usage: wgpu::BufferUsages::VERTEX,
            })
    }
}

/// Build one axis arrow's line-list vertices: the shaft from the origin to `tip`,
/// four lines from `tip` to the arrowhead points `[a1, a2, a3, a4]`, then the ring
/// connecting them (a1-a3, a3-a2, a2-a4, a4-a1).
fn axis_arrow_verts(tip: Vec3, head: [Vec3; 4]) -> Vec<Vertex> {
    let uv = [0.0, 0.0];
    let normal = Vec3::Y;
    let [a1, a2, a3, a4] = head;

    let mut verts = Vec::new();
    verts.push(Vertex::new(Vec3::ZERO, normal, uv));
    verts.push(Vertex::new(tip, normal, uv));
    // Lines from the tip out to each arrowhead point.
    for a in [a1, a2, a3, a4] {
        verts.push(Vertex::new(tip, normal, uv));
        verts.push(Vertex::new(a, normal, uv));
    }
    // Ring connecting the arrowhead points.
    for (p, q) in [(a1, a3), (a3, a2), (a2, a4), (a4, a1)] {
        verts.push(Vertex::new(p, normal, uv));
        verts.push(Vertex::new(q, normal, uv));
    }
    verts
}
