//! Per-frame GPU resource pre-creation: shared resource-tuple type aliases, the
//! default bone uniform, solid entity resources, and the selection outline.
//! Editor overlays live in `draw_overlays`/`draw_path`. Extracted verbatim from
//! `Renderer::render` so each block returns owned resources that outlive the
//! render pass. Behavior unchanged.

use glam::Mat4;
use std::rc::Rc;
use wgpu::util::DeviceExt;

use super::{BoneUniform, EntityUniform, GpuTexture, Renderer};
use crate::scene::Scene;

pub(super) type SolidResource = (
    u32,
    wgpu::Buffer,
    wgpu::Buffer,
    wgpu::BindGroup,
    Rc<GpuTexture>,
    u32,
);
pub(super) type OutlineResource = (u32, wgpu::Buffer, wgpu::Buffer, wgpu::BindGroup, u32);
pub(super) type GridResource = (wgpu::Buffer, wgpu::Buffer, wgpu::BindGroup);
pub(super) type AabbResource = (wgpu::Buffer, wgpu::Buffer, wgpu::Buffer, wgpu::BindGroup);
pub(super) type AxisResource = (usize, wgpu::Buffer, wgpu::Buffer, wgpu::BindGroup);
pub(super) type PathResource = (
    wgpu::Buffer,
    wgpu::Buffer,
    wgpu::Buffer,
    wgpu::BindGroup,
    u32,
);

/// The editor-only overlay resources for one scene pass (selection outline, grid,
/// collider AABBs, axis arrows). Empty outside editor mode. Bundled so the scene pass
/// and the per-camera loop pass a single value instead of four (#93).
#[derive(Default)]
pub(super) struct Overlays {
    pub outline: Option<OutlineResource>,
    pub grid: Option<GridResource>,
    pub aabb: Vec<AabbResource>,
    pub axis: Vec<AxisResource>,
}

impl Renderer {
    pub(super) fn default_bones() -> BoneUniform {
        BoneUniform {
            bones: [Mat4::IDENTITY.to_cols_array(); 64],
        }
    }

    /// Pre-create the editor overlay resources for a pass; all-empty in play mode.
    pub(super) fn precreate_overlays(
        &self,
        scene: &Scene,
        default_bones: &BoneUniform,
        editor_mode: bool,
    ) -> Overlays {
        if !editor_mode {
            return Overlays::default();
        }
        Overlays {
            outline: self.precreate_outline(scene, default_bones),
            grid: self.precreate_grid(default_bones),
            aabb: self.precreate_aabb(scene, default_bones),
            axis: self.precreate_axis_arrows(scene, default_bones),
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn precreate_solid_resources(
        &self,
        scene: &Scene,
        default_bones: &BoneUniform,
        culling_mask: u32,
    ) -> Vec<SolidResource> {
        let mut solid_render_resources = Vec::new();
        for entity in scene.iter() {
            if !entity.active {
                continue;
            }
            // Skip meshes the active camera's culling mask excludes (#92).
            if !crate::scene::layer_in_mask(entity.layer, culling_mask) {
                continue;
            }

            if let Some(mesh) = &entity.mesh {
                if let Some(gpu_mesh) = self.gpu_meshes.get(&entity.id) {
                    // Prepare entity uniform buffer
                    let is_lit = if entity.light.is_some() { 0u32 } else { 1u32 };
                    let model_matrix = scene.compute_world_matrix(entity.id);

                    // Tint is driven by components only — never by entity name. A
                    // game colours its entities via the `texture` component's
                    // `color`; the engine carries no per-name colour assumptions.
                    let color_tint = if let Some(t_comp) = &entity.texture {
                        [t_comp.color[0], t_comp.color[1], t_comp.color[2], 1.0]
                    } else if let Some(health) = &entity.health {
                        if health.is_dead {
                            [0.2, 0.2, 0.2, 1.0]
                        } else {
                            [1.0, 1.0, 1.0, 1.0]
                        }
                    } else {
                        [1.0, 1.0, 1.0, 1.0]
                    };

                    let (metallic, roughness) = if let Some(t_comp) = &entity.texture {
                        (t_comp.metallic, t_comp.roughness)
                    } else {
                        (0.0, 0.5)
                    };

                    let entity_uniform = EntityUniform {
                        model_matrix: model_matrix.to_cols_array(),
                        color_tint,
                        use_texture: if entity.texture.is_some()
                            && !entity.texture.as_ref().unwrap().path.is_empty()
                        {
                            1
                        } else {
                            0
                        },
                        is_lit,
                        metallic,
                        roughness,
                    };

                    let entity_buffer =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Entity Uniform"),
                                contents: bytemuck::bytes_of(&entity_uniform),
                                usage: wgpu::BufferUsages::UNIFORM,
                            });

                    // Upload the mesh's bind-pose bone palette (#79). Skinned
                    // `"Asset"` meshes supply a real palette computed from their
                    // imported skeleton; primitives and static meshes leave an
                    // empty palette, so the GPU bones stay at identity and the
                    // skinning shader is a no-op for them.
                    let mut bones_data = *default_bones;
                    for (i, bone) in mesh.bind_palette.iter().take(64).enumerate() {
                        bones_data.bones[i] = bone.to_cols_array();
                    }

                    let bones_buffer =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Bones Uniform"),
                                contents: bytemuck::bytes_of(&bones_data),
                                usage: wgpu::BufferUsages::UNIFORM,
                            });

                    // Bind Group 1
                    let entity_bind_group =
                        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("Entity Bind Group"),
                            layout: &self.entity_bones_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: entity_buffer.as_entire_binding(),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: bones_buffer.as_entire_binding(),
                                },
                            ],
                        });

                    // Bind Group 2 (Texture)
                    let tex = if let Some(t_comp) = &entity.texture {
                        self.gpu_textures
                            .get(&t_comp.path)
                            .cloned()
                            .unwrap_or_else(|| Rc::clone(&self.default_texture))
                    } else {
                        Rc::clone(&self.default_texture)
                    };

                    solid_render_resources.push((
                        entity.id,
                        entity_buffer,
                        bones_buffer,
                        entity_bind_group,
                        tex,
                        gpu_mesh.num_indices,
                    ));
                }
            }
        }
        solid_render_resources
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn precreate_outline(
        &self,
        scene: &Scene,
        default_bones: &BoneUniform,
    ) -> Option<OutlineResource> {
        let selected_id = scene.selected_entity_id?;
        let entity = scene.get_entity(selected_id)?;
        if !(entity.active && entity.mesh.is_some()) {
            return None;
        }
        let gpu_mesh = self.gpu_meshes.get(&selected_id)?;

        // Scale up the model matrix slightly for the outline hull
        let outline_scale = 1.05;
        let scaled_transform = Mat4::from_scale_rotation_translation(
            entity.transform.scale * outline_scale,
            entity.transform.rotation,
            entity.transform.position,
        );

        let outline_uniform = EntityUniform {
            model_matrix: scaled_transform.to_cols_array(),
            color_tint: [1.0, 0.5, 0.0, 1.0], // Vibrant glowing orange outline
            use_texture: 0,
            is_lit: 0,
            metallic: 0.0,
            roughness: 0.5,
        };

        let outline_ent_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Outline Entity Uniform"),
                contents: bytemuck::bytes_of(&outline_uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let outline_bones_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Outline Bones Uniform"),
                contents: bytemuck::bytes_of(default_bones),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let outline_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Outline Bind Group"),
            layout: &self.entity_bones_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: outline_ent_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: outline_bones_buf.as_entire_binding(),
                },
            ],
        });

        Some((
            selected_id,
            outline_ent_buf,
            outline_bones_buf,
            outline_bind_group,
            gpu_mesh.num_indices,
        ))
    }
}
