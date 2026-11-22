mod axis;
mod batch;
mod game;
mod handles;
mod lights;
mod navmesh;
mod occlusion;
mod probe_ball;
mod probes;
mod wires;

use crate::gpu::groups;
use glam::{Mat4, Vec3};

use crate::draw::resources::{GridResource, Overlays};
use crate::Renderer;
pub(crate) use batch::{OverlayBindings, OverlayDraws};
pub(crate) use navmesh::NavMeshGizmo;
pub use navmesh::{NavMeshPatch, NavMeshRevision, NavMeshSource};
pub(crate) use probe_ball::{ball_pipeline, BallResource};
use rusty_components::mesh::Vertex;
pub(crate) use wires::ColliderScope;

impl Renderer {
    pub(crate) fn precreate_grid(&self, draws: &mut OverlayDraws) -> Option<GridResource> {
        self.grid_vertex_buffer.as_ref()?;
        Some(draws.push(Mat4::IDENTITY, [0.15, 0.15, 0.22, 1.0]))
    }

    pub(crate) fn draw_editor_overlays<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        overlays: &'a Overlays,
    ) {
        let Some(bindings) = &overlays.bindings else {
            return;
        };
        // Balls first, writing depth, then the translucent navmesh, then the lines that test both.
        self.draw_probe_balls(render_pass, bindings, &overlays.balls);
        self.draw_navmesh(render_pass, bindings, overlays.navmesh);
        if let (Some(slot), Some(grid_buf)) = (overlays.grid, &self.grid_vertex_buffer) {
            render_pass.set_vertex_buffer(0, grid_buf.slice(..));
            bindings.bind(render_pass, slot);
            render_pass.set_bind_group(groups::MATERIAL, &self.default_material_bind_group, &[]);
            render_pass.draw(0..self.grid_count, 0..1);
        }
        for (aabb_wire_buffer, slot) in &overlays.aabb {
            render_pass.set_vertex_buffer(0, aabb_wire_buffer.slice(..));
            bindings.bind(render_pass, *slot);
            render_pass.draw(0..24, 0..1);
        }
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
        let lists = [&overlays.probes, &overlays.wires];
        if lists.iter().any(|list| list.is_some()) {
            render_pass.set_pipeline(self.vertex_line_pipeline.get());
        }
        for (buffer, slot, count) in lists.into_iter().flatten() {
            render_pass.set_vertex_buffer(0, buffer.slice(..));
            bindings.bind(render_pass, *slot);
            render_pass.draw(0..*count, 0..1);
        }
    }
}

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

    line_vertices.push(Vertex::new(p000, normal, uv));
    line_vertices.push(Vertex::new(p100, normal, uv));
    line_vertices.push(Vertex::new(p100, normal, uv));
    line_vertices.push(Vertex::new(p101, normal, uv));
    line_vertices.push(Vertex::new(p101, normal, uv));
    line_vertices.push(Vertex::new(p001, normal, uv));
    line_vertices.push(Vertex::new(p001, normal, uv));
    line_vertices.push(Vertex::new(p000, normal, uv));

    line_vertices.push(Vertex::new(p010, normal, uv));
    line_vertices.push(Vertex::new(p110, normal, uv));
    line_vertices.push(Vertex::new(p110, normal, uv));
    line_vertices.push(Vertex::new(p111, normal, uv));
    line_vertices.push(Vertex::new(p111, normal, uv));
    line_vertices.push(Vertex::new(p011, normal, uv));
    line_vertices.push(Vertex::new(p011, normal, uv));
    line_vertices.push(Vertex::new(p010, normal, uv));

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
