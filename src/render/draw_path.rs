//! Pre-creation of selected-entity axis arrows and the navigation path line.
//! Extracted verbatim from `Renderer::render` (behavior unchanged).

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use super::draw_resources::{AxisResource, PathResource};
use super::mesh::Vertex;
use super::{EntityUniform, Renderer};
use crate::scene::Scene;

impl Renderer {
    pub(super) fn precreate_axis_arrows(&self, scene: &Scene) -> Vec<AxisResource> {
        let mut axis_arrow_resources = Vec::new();
        let Some(selected_id) = scene.selected_entity_id else {
            return axis_arrow_resources;
        };
        if scene.get_entity(selected_id).is_none() {
            return axis_arrow_resources;
        }
        let world_matrix = scene.compute_world_matrix(selected_id);
        let world_pos = world_matrix.col(3).truncate();
        let arrow_model_matrix = Mat4::from_translation(world_pos);

        let colors = [
            [1.0, 0.1, 0.1, 1.0], // X: Red
            [0.1, 0.9, 0.1, 1.0], // Y: Green
            [0.1, 0.4, 1.0, 1.0], // Z: Blue
        ];

        for (i, color) in colors.iter().enumerate() {
            axis_arrow_resources.push(self.build_axis_arrow(i, arrow_model_matrix, *color));
        }
        axis_arrow_resources
    }

    /// Build one axis-arrow's uniform buffer + bind group (bound against the shared
    /// identity bone palette).
    fn build_axis_arrow(
        &self,
        i: usize,
        arrow_model_matrix: Mat4,
        color: [f32; 4],
    ) -> AxisResource {
        let entity_uniform = EntityUniform {
            model_matrix: arrow_model_matrix.to_cols_array(),
            color_tint: color,
            use_texture: 0,
            is_lit: 0,
            metallic: 0.0,
            roughness: 0.5,
            use_metallic_map: 0,
            use_roughness_map: 0,
            use_normal_map: 0,
            use_emissive_map: 0,
            emissive: [0.0; 4],
        };

        let entity_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Axis Arrow Uniform"),
                contents: bytemuck::bytes_of(&entity_uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bind_group =
            self.entity_bind_group("Axis Arrow", &entity_buf, self.shared_bones_buffer());
        (i, entity_buf, bind_group)
    }

    pub(super) fn precreate_path(&self, pathfinding_points: &[Vec3]) -> Option<PathResource> {
        if pathfinding_points.len() < 2 {
            return None;
        }
        let mut path_vertices = Vec::new();
        let normal = Vec3::Y;
        let uv = [0.0, 0.0];

        for i in 0..pathfinding_points.len() - 1 {
            // Lift points slightly above floor grid to prevent z-fighting
            let p1 = pathfinding_points[i] + Vec3::new(0.0, 0.05, 0.0);
            let p2 = pathfinding_points[i + 1] + Vec3::new(0.0, 0.05, 0.0);

            path_vertices.push(Vertex::new(p1, normal, uv));
            path_vertices.push(Vertex::new(p2, normal, uv));
        }

        let path_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Path Vertices"),
                contents: bytemuck::cast_slice(&path_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let (entity_buf, path_bind_group) = self.build_path_bindings();
        Some((
            path_buffer,
            entity_buf,
            path_bind_group,
            path_vertices.len() as u32,
        ))
    }

    /// Build the path line's uniform buffer + bind group (bound against the shared
    /// identity bone palette).
    fn build_path_bindings(&self) -> (wgpu::Buffer, wgpu::BindGroup) {
        let entity_uniform = EntityUniform {
            model_matrix: Mat4::IDENTITY.to_cols_array(),
            color_tint: [0.0, 1.0, 0.3, 1.0], // Neon green pathline
            use_texture: 0,
            is_lit: 0,
            metallic: 0.0,
            roughness: 0.5,
            use_metallic_map: 0,
            use_roughness_map: 0,
            use_normal_map: 0,
            use_emissive_map: 0,
            emissive: [0.0; 4],
        };

        let entity_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::bytes_of(&entity_uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let path_bind_group =
            self.entity_bind_group("Path", &entity_buf, self.shared_bones_buffer());
        (entity_buf, path_bind_group)
    }
}
