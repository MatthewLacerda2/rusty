//! Pre-creation of editor overlay resources: the selection outline, floor grid, and
//! per-collider AABB wireframes. Extracted from `Renderer::render` (behavior
//! unchanged).

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::render::draw::resources::{AabbResource, AxisResource, GridResource, OutlineResource};
use crate::render::gpu::mesh::Vertex;
use crate::render::{EntityUniform, MeshId, Renderer};
use crate::scene::Scene;

impl Renderer {
    pub(crate) fn precreate_grid(&self) -> Option<GridResource> {
        let _grid_buf = self.grid_vertex_buffer.as_ref()?;
        let grid_uniform = EntityUniform {
            model_matrix: Mat4::IDENTITY.to_cols_array(),
            color_tint: [0.15, 0.15, 0.22, 1.0], // Neon slate blue grid
            use_texture: 0,
            is_lit: 0,
            metallic: 0.0,
            roughness: 0.5,
            use_metallic_map: 0,
            use_roughness_map: 0,
            use_normal_map: 0,
            use_emissive_map: 0,
            emissive: [0.0; 4],
            use_sh: 0,
            use_cutout: 0,
            alpha_cutoff: 0.0,
            _sh_pad: 0,
            sh: [[0.0; 4]; 9],
        };
        let grid_buf_unif = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Grid Unif"),
                contents: bytemuck::bytes_of(&grid_uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let grid_bind_group =
            self.entity_bind_group("Grid", &grid_buf_unif, self.shared_bones_buffer());
        Some((grid_buf_unif, grid_bind_group))
    }

    pub(crate) fn precreate_aabb(&self, scene: &Scene) -> Vec<AabbResource> {
        let mut aabb_resources = Vec::new();
        for id in scene.world.ids_with_collider() {
            if !scene.world.is_active(id) {
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

            let line_vertices = aabb_wireframe(col.aabb_min, col.aabb_max);
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
    /// renderer's shared identity bone palette).
    fn build_aabb_bindings(&self, tint_color: [f32; 4]) -> (wgpu::Buffer, wgpu::BindGroup) {
        let entity_uniform = EntityUniform {
            model_matrix: Mat4::IDENTITY.to_cols_array(), // Vertices are already in world space
            color_tint: tint_color,
            use_texture: 0,
            is_lit: 0,
            metallic: 0.0,
            roughness: 0.5,
            use_metallic_map: 0,
            use_roughness_map: 0,
            use_normal_map: 0,
            use_emissive_map: 0,
            emissive: [0.0; 4],
            use_sh: 0,
            use_cutout: 0,
            alpha_cutoff: 0.0,
            _sh_pad: 0,
            sh: [[0.0; 4]; 9],
        };

        let entity_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::bytes_of(&entity_uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let col_bind_group =
            self.entity_bind_group("AABB", &entity_buf, self.shared_bones_buffer());
        (entity_buf, col_bind_group)
    }

    pub(crate) fn precreate_outline(&self, scene: &Scene) -> Option<OutlineResource> {
        // (uniform literal split into `outline_uniform` below for the 50-line fn cap)
        let selected_id = scene.selected_entity_id?;
        let mesh = scene.world.mesh(selected_id)?;
        let mesh_id = MeshId::from_mesh(&mesh);
        if !scene.world.is_active(selected_id) {
            return None;
        }
        let gpu_mesh = self.gpu_meshes.get(&mesh_id)?;

        let transform = scene
            .world
            .transform(selected_id)
            .expect("mandatory Transform");
        // Scale up the model matrix slightly for the outline hull
        let outline_scale = 1.05;
        let scaled_transform = Mat4::from_scale_rotation_translation(
            transform.scale * outline_scale,
            transform.rotation,
            transform.position,
        );

        let outline_uniform = outline_uniform(scaled_transform);

        let outline_ent_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Outline Entity Uniform"),
                contents: bytemuck::bytes_of(&outline_uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let outline_bind_group = self.entity_bind_group(
            "Outline Bind Group",
            &outline_ent_buf,
            self.shared_bones_buffer(),
        );

        Some((
            selected_id,
            mesh_id,
            outline_ent_buf,
            outline_bind_group,
            gpu_mesh.num_indices,
        ))
    }

    /// Draw the editor overlays (floor grid, collider AABBs, selected-entity axis
    /// arrows) into the scene pass. Editor-mode only; called from `record_scene_pass`.
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
                render_pass.set_bind_group(1, grid_bind_group, &[]);
                render_pass.set_bind_group(2, &self.default_material_bind_group, &[]);
                render_pass.draw(0..self.grid_count, 0..1);
            }
        }
        // B. AABB outlines for active colliders
        for (aabb_wire_buffer, _entity_buf, col_bind_group) in aabb_resources {
            render_pass.set_vertex_buffer(0, aabb_wire_buffer.slice(..));
            render_pass.set_bind_group(1, col_bind_group, &[]);
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
                render_pass.set_bind_group(1, bind_group, &[]);
                render_pass.draw(0..self.axis_count, 0..1);
            }
        }
    }
}

/// The fixed outline-hull uniform (orange tint, unlit) at `model` — split out of
/// `precreate_outline` for the 50-line fn cap.
fn outline_uniform(model: Mat4) -> EntityUniform {
    EntityUniform {
        model_matrix: model.to_cols_array(),
        color_tint: [1.0, 0.5, 0.0, 1.0], // Vibrant glowing orange outline
        use_texture: 0,
        is_lit: 0,
        metallic: 0.0,
        roughness: 0.5,
        use_metallic_map: 0,
        use_roughness_map: 0,
        use_normal_map: 0,
        use_emissive_map: 0,
        emissive: [0.0; 4],
        use_sh: 0,
        use_cutout: 0,
        alpha_cutoff: 0.0,
        _sh_pad: 0,
        sh: [[0.0; 4]; 9],
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
