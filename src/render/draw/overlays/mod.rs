//! Pre-creation of editor overlay resources: the floor grid and per-collider AABB
//! wireframes (the selection outline is its own pass, `passes::outline`), and the
//! pass that draws them over the finished image ([`handles`], #1224).

mod axis;
mod handles;
mod probes;

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::components::mesh::Vertex;
use crate::render::draw::resources::{AabbResource, AxisResource, GridResource};
use crate::render::gpu::draw_buffers::group1;
use crate::render::{DrawUniform, MaterialData, Renderer};
use crate::scene::Scene;

impl Renderer {
    /// An editor overlay's group 1: its own per-draw uniform buffer (`model_matrix`
    /// placing it, record 0) over a one-record material array of `tint`
    /// ([`MaterialData::flat`]), the shared identity bones, and the shared one-element
    /// identity instance array — an overlay is one draw of instance 0, its transform
    /// carried in the uniform. Bound at offset `[0]`. Returns the uniform buffer with
    /// the group.
    pub(crate) fn overlay_bindings(
        &self,
        label: &str,
        model_matrix: glam::Mat4,
        tint: [f32; 4],
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        let init = |contents: &[u8], usage| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents,
                    usage,
                })
        };
        let draw = DrawUniform {
            model_matrix: model_matrix.to_cols_array(),
            bone_base: 0,
            material: 0,
            _pad: [0; 2],
        };
        let uniform = init(bytemuck::bytes_of(&draw), wgpu::BufferUsages::UNIFORM);
        let record = MaterialData::flat(tint);
        let material = init(bytemuck::bytes_of(&record), wgpu::BufferUsages::STORAGE);
        let group = group1(
            &self.device,
            &self.entity_bones_layout,
            label,
            [
                &uniform,
                self.shared_bones_buffer(),
                self.draw_buffers.identity_instance(),
                &material,
            ],
        );
        (uniform, group)
    }

    pub(crate) fn precreate_grid(&self) -> Option<GridResource> {
        let _grid_buf = self.grid_vertex_buffer.as_ref()?;
        // Neon slate blue grid.
        let (grid_buf_unif, grid_bind_group) =
            self.overlay_bindings("Grid", Mat4::IDENTITY, [0.15, 0.15, 0.22, 1.0]);
        Some((grid_buf_unif, grid_bind_group))
    }

    pub(crate) fn precreate_aabb(&self, scene: &Scene) -> Vec<AabbResource> {
        let mut aabb_resources = Vec::new();
        for id in scene.world.ids_with_collider() {
            if !scene.world.is_active_in_hierarchy(id) {
                continue;
            }
            if scene.selected_entity_id == Some(id) {
                continue;
            }
            let col = scene
                .world
                .collider(id)
                .expect("id came from ids_with_collider");
            if !col.active {
                continue;
            }

            drop(col);
            let Some((min, max)) = scene.world.collider_bounds(id) else {
                continue;
            };
            let line_vertices = aabb_wireframe(min, max);
            let aabb_wire_buffer =
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: bytemuck::cast_slice(&line_vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });

            // Bright glowing green if selected, cyan otherwise
            let is_selected = scene.selected_entity_id == Some(id);
            let tint_color = if is_selected {
                [0.0, 1.0, 0.4, 1.0]
            } else {
                [0.0, 0.8, 1.0, 0.8]
            };

            let (entity_buf, col_bind_group) = self.build_aabb_bindings(tint_color);
            aabb_resources.push((aabb_wire_buffer, entity_buf, col_bind_group));
        }
        aabb_resources
    }

    /// Build an AABB wireframe's uniform buffer + bind group (bound against the
    /// renderer's shared identity bone palette). Its vertices are already in world space.
    fn build_aabb_bindings(&self, tint_color: [f32; 4]) -> (wgpu::Buffer, wgpu::BindGroup) {
        self.overlay_bindings("AABB", Mat4::IDENTITY, tint_color)
    }

    /// Draw the editor overlays (floor grid, collider AABBs, selected-entity axis
    /// arrows) into the handles pass. Editor-mode only; called from `draw_handles`.
    pub(crate) fn draw_editor_overlays<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        grid_resources: &'a Option<GridResource>,
        aabb_resources: &'a [AabbResource],
        axis_arrow_resources: &'a [AxisResource],
    ) {
        // A. Floor grid
        if let Some((_grid_buf_unif, grid_bind_group)) = grid_resources {
            if let Some(grid_buf) = &self.grid_vertex_buffer {
                render_pass.set_vertex_buffer(0, grid_buf.slice(..));
                render_pass.set_bind_group(1, grid_bind_group, &[0]);
                render_pass.set_bind_group(2, &self.default_material_bind_group, &[]);
                render_pass.draw(0..self.grid_count, 0..1);
            }
        }
        // B. AABB outlines for active colliders
        for (aabb_wire_buffer, _entity_buf, col_bind_group) in aabb_resources {
            render_pass.set_vertex_buffer(0, aabb_wire_buffer.slice(..));
            render_pass.set_bind_group(1, col_bind_group, &[0]);
            render_pass.draw(0..24, 0..1);
        }
        // C. Axis arrows for the selected entity
        for (i, _entity_buf, bind_group) in axis_arrow_resources {
            let buffer = match i {
                0 => &self.axis_x_buffer,
                1 => &self.axis_y_buffer,
                2 => &self.axis_z_buffer,
                _ => &None,
            };
            if let Some(buf) = buffer {
                render_pass.set_vertex_buffer(0, buf.slice(..));
                render_pass.set_bind_group(1, bind_group, &[0]);
                render_pass.draw(0..self.axis_count, 0..1);
            }
        }
    }
}

/// Generate the 24 line vertices (12 edges) of an axis-aligned box's wireframe.
/// Shared with the reflection-probe parallax-box gizmo (#284), which reuses this
/// exact helper rather than duplicating the edge layout.
pub(crate) fn aabb_wireframe(min: Vec3, max: Vec3) -> Vec<Vertex> {
    let mut line_vertices = Vec::new();
    let normal = Vec3::Y;
    let uv = [0.0, 0.0];

    let p000 = Vec3::new(min.x, min.y, min.z);
    let p100 = Vec3::new(max.x, min.y, min.z);
    let p010 = Vec3::new(min.x, max.y, min.z);
    let p110 = Vec3::new(max.x, max.y, min.z);
    let p001 = Vec3::new(min.x, min.y, max.z);
    let p101 = Vec3::new(max.x, min.y, max.z);
    let p011 = Vec3::new(min.x, max.y, max.z);
    let p111 = Vec3::new(max.x, max.y, max.z);

    // Bottom face
    line_vertices.push(Vertex::new(p000, normal, uv));
    line_vertices.push(Vertex::new(p100, normal, uv));
    line_vertices.push(Vertex::new(p100, normal, uv));
    line_vertices.push(Vertex::new(p101, normal, uv));
    line_vertices.push(Vertex::new(p101, normal, uv));
    line_vertices.push(Vertex::new(p001, normal, uv));
    line_vertices.push(Vertex::new(p001, normal, uv));
    line_vertices.push(Vertex::new(p000, normal, uv));

    // Top face
    line_vertices.push(Vertex::new(p010, normal, uv));
    line_vertices.push(Vertex::new(p110, normal, uv));
    line_vertices.push(Vertex::new(p110, normal, uv));
    line_vertices.push(Vertex::new(p111, normal, uv));
    line_vertices.push(Vertex::new(p111, normal, uv));
    line_vertices.push(Vertex::new(p011, normal, uv));
    line_vertices.push(Vertex::new(p011, normal, uv));
    line_vertices.push(Vertex::new(p010, normal, uv));

    // Verticals connecting bottom and top
    line_vertices.push(Vertex::new(p000, normal, uv));
    line_vertices.push(Vertex::new(p010, normal, uv));
    line_vertices.push(Vertex::new(p100, normal, uv));
    line_vertices.push(Vertex::new(p110, normal, uv));
    line_vertices.push(Vertex::new(p101, normal, uv));
    line_vertices.push(Vertex::new(p111, normal, uv));
    line_vertices.push(Vertex::new(p001, normal, uv));
    line_vertices.push(Vertex::new(p011, normal, uv));

    line_vertices
}
