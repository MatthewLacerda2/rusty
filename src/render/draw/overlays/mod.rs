//! Pre-creation of editor overlay resources: the floor grid and per-collider AABB
//! wireframes (the selection outline is its own pass, `passes::outline`), and the
//! pass that draws them over the finished image ([`handles`], #1224).

mod axis;
mod batch;
mod handles;
mod lights;
mod probes;

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::components::mesh::Vertex;
use crate::render::draw::resources::{AabbResource, GridResource, Overlays};
use crate::render::Renderer;
use crate::scene::Scene;
pub(crate) use batch::{OverlayBindings, OverlayDraws};

impl Renderer {
    pub(crate) fn precreate_grid(&self, draws: &mut OverlayDraws) -> Option<GridResource> {
        self.grid_vertex_buffer.as_ref()?;
        // Neon slate blue grid.
        Some(draws.push(Mat4::IDENTITY, [0.15, 0.15, 0.22, 1.0]))
    }

    pub(crate) fn precreate_aabb(
        &self,
        scene: &Scene,
        draws: &mut OverlayDraws,
    ) -> Vec<AabbResource> {
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

            // Its vertices are already in world space.
            let slot = draws.push(Mat4::IDENTITY, tint_color);
            aabb_resources.push((aabb_wire_buffer, slot));
        }
        aabb_resources
    }

    /// Draw the editor overlays (floor grid, collider AABBs, selected-entity axis
    /// arrows) into the handles pass, each at its slot of `overlays`' one group 1.
    /// Editor-mode only; called from `draw_handles`.
    pub(crate) fn draw_editor_overlays<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        overlays: &'a Overlays,
    ) {
        let Some(bindings) = &overlays.bindings else {
            return;
        };
        // A. Floor grid
        if let (Some(slot), Some(grid_buf)) = (overlays.grid, &self.grid_vertex_buffer) {
            render_pass.set_vertex_buffer(0, grid_buf.slice(..));
            bindings.bind(render_pass, slot);
            render_pass.set_bind_group(2, &self.default_material_bind_group, &[]);
            render_pass.draw(0..self.grid_count, 0..1);
        }
        // B. AABB outlines for active colliders
        for (aabb_wire_buffer, slot) in &overlays.aabb {
            render_pass.set_vertex_buffer(0, aabb_wire_buffer.slice(..));
            bindings.bind(render_pass, *slot);
            render_pass.draw(0..24, 0..1);
        }
        // C. Axis arrows for the selected entity
        for &(i, slot) in &overlays.axis {
            let buffer = match i {
                0 => &self.axis_x_buffer,
                1 => &self.axis_y_buffer,
                2 => &self.axis_z_buffer,
                _ => &None,
            };
            if let Some(buf) = buffer {
                render_pass.set_vertex_buffer(0, buf.slice(..));
                bindings.bind(render_pass, slot);
                render_pass.draw(0..self.axis_count, 0..1);
            }
        }
        // D. Light- & reflection-probe gizmos (#284) and Area lights (#1674).
        self.draw_probe_overlays(render_pass, bindings, &overlays.probes);
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
