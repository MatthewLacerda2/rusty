use glam::Vec3;
use wgpu::util::DeviceExt;

use std::collections::BTreeMap;

use super::{GpuMesh, MeshId, Renderer, SkinBounds};
use crate::components::mesh::{MeshRevision, Vertex};
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

/// One mesh a geometry slot could be uploaded from.
pub(crate) struct UploadCandidate {
    pub id: MeshId,
    pub revision: MeshRevision,
    pub entity: u32,
    /// Has vertices and indices: an empty mesh never replaces a slot's buffers.
    pub has_geometry: bool,
}

/// Which slots to (re-)upload, each from which mesh (#1540): a slot whose resident
/// buffers hold a revision at least as new as a candidate's skips it; otherwise the
/// newest candidate with geometry under the slot wins. `resident` is the revision a
/// renderer uploaded per slot, `None` when nothing is.
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
    /// Upload every entity's mesh once per unique mesh-asset identity (#127): N
    /// entities sharing a primitive/asset collapse to a single buffer pair. An
    /// inactive entity's mesh is uploaded too, as Unity keeps it loaded: only what
    /// is drawn follows activity, so re-enabling a holstered weapon doesn't stall on
    /// uploads (#1363). An identity is (re-)uploaded when it is not resident or a
    /// mesh under it carries a newer [`MeshRevision`] than the resident buffers, from
    /// the newest such mesh, once. Reading the revision changes nothing, so every
    /// renderer drawing the scene sees the same edit (#1540).
    pub(crate) fn upload_scene_meshes(&mut self, scene: &Scene) {
        let candidates = scene.world.ids_with_mesh().into_iter().map(|entity| {
            let mesh = scene
                .world
                .mesh(entity)
                .expect("id came from ids_with_mesh");
            UploadCandidate {
                id: MeshId::from_mesh(&mesh),
                revision: mesh.revision,
                entity,
                has_geometry: !mesh.vertices.is_empty() && !mesh.indices.is_empty(),
            }
        });
        let resident = |id: &MeshId| self.gpu_meshes.get(id).map(|g| g.revision);
        for (id, (revision, entity)) in stale_uploads(candidates, resident) {
            let mesh = scene.world.mesh(entity).expect("collected above");
            self.update_gpu_mesh(id, revision, &mesh.vertices, &mesh.indices);
        }
    }

    /// Uploads mesh data to the GPU, caching it under its mesh-asset identity
    /// (#127) so geometry shared by many entities is stored once. When a buffer
    /// for this `MeshId` is already resident and large enough, the data is written
    /// in place (`queue.write_buffer`) rather than reallocating a fresh buffer.
    /// `revision` is the geometry's [`MeshRevision`], recorded so a newer one
    /// replaces it.
    pub fn update_gpu_mesh(
        &mut self,
        id: MeshId,
        revision: MeshRevision,
        vertices: &[Vertex],
        indices: &[u32],
    ) {
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
        let skin_bounds = SkinBounds::from_vertices(vertices);

        // Re-use resident buffers when the new data still fits (a newer revision
        // of geometry already resident under this id). Both buffers carry COPY_DST
        // so they can be written without reallocation.
        if let Some(existing) = self.gpu_meshes.get_mut(&id) {
            if existing.vertex_buffer.size() >= v_bytes.len() as u64
                && existing.index_buffer.size() >= i_bytes.len() as u64
            {
                existing.num_indices = num_indices;
                existing.local_aabb = local_aabb;
                existing.skin_bounds = skin_bounds;
                existing.revision = revision;
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
                skin_bounds,
                revision,
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

#[cfg(test)]
#[path = "debug_meshes_tests.rs"]
mod tests;
