//! Pre-creation of selected-entity axis arrows and the navigation path line.
//! Extracted verbatim from `Renderer::render` (behavior unchanged).

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use super::draw_resources::{AxisResource, PathResource};
use super::mesh::Vertex;
use super::{BoneUniform, EntityUniform, Renderer};
use crate::scene::Scene;

impl Renderer {
    #[allow(clippy::too_many_lines)]
    pub(super) fn precreate_axis_arrows(
        &self,
        scene: &Scene,
        default_bones: &BoneUniform,
    ) -> Vec<AxisResource> {
        let mut axis_arrow_resources = Vec::new();
        if let Some(selected_id) = scene.selected_entity_id {
            if scene.get_entity(selected_id).is_some() {
                let world_matrix = scene.compute_world_matrix(selected_id);
                let world_pos = world_matrix.col(3).truncate();
                let arrow_model_matrix = Mat4::from_translation(world_pos);

                let colors = [
                    [1.0, 0.1, 0.1, 1.0], // X: Red
                    [0.1, 0.9, 0.1, 1.0], // Y: Green
                    [0.1, 0.4, 1.0, 1.0], // Z: Blue
                ];

                for (i, color) in colors.iter().enumerate() {
                    let entity_uniform = EntityUniform {
                        model_matrix: arrow_model_matrix.to_cols_array(),
                        color_tint: *color,
                        use_texture: 0,
                        is_lit: 0,
                        metallic: 0.0,
                        roughness: 0.5,
                    };

                    let entity_buf =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Axis Arrow Uniform"),
                                contents: bytemuck::bytes_of(&entity_uniform),
                                usage: wgpu::BufferUsages::UNIFORM,
                            });

                    let default_bones_buf =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Axis Arrow Bones"),
                                contents: bytemuck::bytes_of(default_bones),
                                usage: wgpu::BufferUsages::UNIFORM,
                            });

                    let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Axis Arrow Bind Group"),
                        layout: &self.entity_bones_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: entity_buf.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: default_bones_buf.as_entire_binding(),
                            },
                        ],
                    });

                    axis_arrow_resources.push((i, entity_buf, default_bones_buf, bind_group));
                }
            }
        }
        axis_arrow_resources
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn precreate_path(
        &self,
        pathfinding_points: &[Vec3],
        default_bones: &BoneUniform,
    ) -> Option<PathResource> {
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

        let entity_uniform = EntityUniform {
            model_matrix: Mat4::IDENTITY.to_cols_array(),
            color_tint: [0.0, 1.0, 0.3, 1.0], // Neon green pathline
            use_texture: 0,
            is_lit: 0,
            metallic: 0.0,
            roughness: 0.5,
        };

        let entity_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::bytes_of(&entity_uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let default_bones_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::bytes_of(default_bones),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let path_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.entity_bones_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: entity_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: default_bones_buf.as_entire_binding(),
                },
            ],
        });

        Some((
            path_buffer,
            entity_buf,
            default_bones_buf,
            path_bind_group,
            path_vertices.len() as u32,
        ))
    }
}
