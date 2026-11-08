//! Pre-creation of editor overlay resources: the floor grid, the selected camera's
//! occlusion view (`occlusion`, #1714), the selection's axis arrows and the probe
//! gizmos the Scene view's set leaves on (#1909) (the selection outline is its own
//! pass, `passes::outline`), the colliders' real shapes and the `Debug.Draw*` lines
//! ([`wires`], #1882, also over a game frame with its gizmos on), the baked navmesh
//! ([`navmesh`], #1910), and the pass that
//! draws them over the finished image ([`handles`], #1224).

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

use glam::{Mat4, Vec3};

use crate::components::mesh::Vertex;
use crate::render::draw::resources::{GridResource, Overlays};
use crate::render::Renderer;
pub(crate) use batch::{OverlayBindings, OverlayDraws};
pub(crate) use navmesh::NavMeshGizmo;
pub use navmesh::{NavMeshPatch, NavMeshRevision, NavMeshSource};
pub(crate) use probe_ball::{ball_pipeline, BallResource};
pub(crate) use wires::ColliderScope;

impl Renderer {
    pub(crate) fn precreate_grid(&self, draws: &mut OverlayDraws) -> Option<GridResource> {
        self.grid_vertex_buffer.as_ref()?;
        // Neon slate blue grid.
        Some(draws.push(Mat4::IDENTITY, [0.15, 0.15, 0.22, 1.0]))
    }

    /// Draw the editor overlays (floor grid, the occlusion view's boxes, selected-entity
    /// axis arrows, probe balls and gizmos) into the handles pass, each at its slot of `overlays`' one group 1.
    /// Editor-mode only; called from `draw_handles`.
    pub(crate) fn draw_editor_overlays<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        overlays: &'a Overlays,
    ) {
        let Some(bindings) = &overlays.bindings else {
            return;
        };
        // The reflection probes' mirror balls (#1911) first: they write depth, so the
        // lines drawn after them hide behind a ball as behind a solid.
        self.draw_probe_balls(render_pass, bindings, &overlays.balls);
        // The navmesh (#1910): translucent, after the opaque balls, under the lines.
        self.draw_navmesh(render_pass, bindings, overlays.navmesh);
        // A. Floor grid
        if let (Some(slot), Some(grid_buf)) = (overlays.grid, &self.grid_vertex_buffer) {
            render_pass.set_vertex_buffer(0, grid_buf.slice(..));
            bindings.bind(render_pass, slot);
            render_pass.set_bind_group(2, &self.default_material_bind_group, &[]);
            render_pass.draw(0..self.grid_count, 0..1);
        }
        // B. Box outlines: the selected camera's occlusion view (#1714)
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
        // E. Collider shapes and `Debug.Draw*` lines (#1882): one draw, each vertex
        // its own colour, so through the vertex-colour line pipeline. Last: it
        // leaves that pipeline set.
        if let Some((buffer, slot, count)) = &overlays.wires {
            render_pass.set_pipeline(self.vertex_line_pipeline.get());
            render_pass.set_vertex_buffer(0, buffer.slice(..));
            bindings.bind(render_pass, *slot);
            render_pass.draw(0..*count, 0..1);
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
