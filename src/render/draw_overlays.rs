//! Pre-creation of editor overlay resources: floor grid and per-collider AABB
//! wireframes. Extracted verbatim from `Renderer::render` (behavior unchanged).

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use super::draw_resources::{AabbResource, GridResource};
use super::mesh::Vertex;
use super::{BoneUniform, EntityUniform, Renderer};
use crate::scene::Scene;

impl Renderer {
    pub(super) fn precreate_grid(&self, default_bones: &BoneUniform) -> Option<GridResource> {
        let _grid_buf = self.grid_vertex_buffer.as_ref()?;
        let grid_uniform = EntityUniform {
            model_matrix: Mat4::IDENTITY.to_cols_array(),
            color_tint: [0.15, 0.15, 0.22, 1.0], // Neon slate blue grid
            use_texture: 0,
            is_lit: 0,
            metallic: 0.0,
            roughness: 0.5,
        };
        let grid_buf_unif = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Grid Unif"),
                contents: bytemuck::bytes_of(&grid_uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let default_bones_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Grid Bones Unif"),
                contents: bytemuck::bytes_of(default_bones),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let grid_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.entity_bones_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: grid_buf_unif.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: default_bones_buf.as_entire_binding(),
                },
            ],
        });
        Some((grid_buf_unif, default_bones_buf, grid_bind_group))
    }

    pub(super) fn precreate_aabb(
        &self,
        scene: &Scene,
        default_bones: &BoneUniform,
    ) -> Vec<AabbResource> {
        let mut aabb_resources = Vec::new();
        for entity in scene.iter() {
            if !entity.active {
                continue;
            }
            if scene.selected_entity_id == Some(entity.id) {
                continue;
            }
            if let Some(col) = &entity.collider {
                if !col.active {
                    continue;
                }

                let min = col.aabb_min;
                let max = col.aabb_max;

                // Generate wireframe box lines (12 lines = 24 vertices)
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

                let aabb_wire_buffer =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: None,
                            contents: bytemuck::cast_slice(&line_vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });

                // Bright glowing green if selected, cyan otherwise
                let is_selected = scene.selected_entity_id == Some(entity.id);
                let tint_color = if is_selected {
                    [0.0, 1.0, 0.4, 1.0]
                } else {
                    [0.0, 0.8, 1.0, 0.8]
                };

                let entity_uniform = EntityUniform {
                    model_matrix: Mat4::IDENTITY.to_cols_array(), // Vertices are already in world space
                    color_tint: tint_color,
                    use_texture: 0,
                    is_lit: 0,
                    metallic: 0.0,
                    roughness: 0.5,
                };

                let entity_buf =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: None,
                            contents: bytemuck::bytes_of(&entity_uniform),
                            usage: wgpu::BufferUsages::UNIFORM,
                        });
                let default_bones_buf =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: None,
                            contents: bytemuck::bytes_of(default_bones),
                            usage: wgpu::BufferUsages::UNIFORM,
                        });
                let col_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
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

                aabb_resources.push((
                    aabb_wire_buffer,
                    entity_buf,
                    default_bones_buf,
                    col_bind_group,
                ));
            }
        }
        aabb_resources
    }
}
