//! Per-frame GPU resource pre-creation: shared resource-tuple type aliases, the
//! default bone uniform, solid entity resources, and the selection outline.
//! Editor overlays live in `draw_overlays`/`draw_path`. Extracted verbatim from
//! `Renderer::render` so each block returns owned resources that outlive the
//! render pass. Behavior unchanged.

use glam::Mat4;
use std::rc::Rc;
use wgpu::util::DeviceExt;

use super::{BoneUniform, EntityUniform, GpuTexture, MeshId, Renderer};
use crate::components::MaterialAsset;
use crate::scene::Scene;

// The leading `u32` is the entity id (identity — e.g. matching the selected entity
// for its outline); `MeshId` is the geometry key the render pass resolves the shared
// vertex/index buffers through (#127). The second `BindGroup` is the per-entity
// group(2) material bind group (albedo + metallic + roughness maps), assembled from
// the entity's resolved textures against `material_layout` (#202).
pub(super) type SolidResource = (
    u32,
    MeshId,
    wgpu::Buffer,
    wgpu::Buffer,
    wgpu::BindGroup,
    wgpu::BindGroup,
    u32,
);
pub(super) type OutlineResource = (
    u32,
    MeshId,
    wgpu::Buffer,
    wgpu::Buffer,
    wgpu::BindGroup,
    u32,
);
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
            if let Some(res) = self.build_solid_resource(scene, &entity, default_bones) {
                solid_render_resources.push(res);
            }
        }
        solid_render_resources
    }

    /// Build the GPU resources for one solid entity, or `None` if its mesh is not
    /// resident on the GPU yet.
    fn build_solid_resource(
        &self,
        scene: &Scene,
        entity: &crate::components::Entity,
        default_bones: &BoneUniform,
    ) -> Option<SolidResource> {
        let mesh = entity.mesh.as_ref()?;
        let mesh_id = MeshId::from_mesh(mesh);
        let gpu_mesh = self.gpu_meshes.get(&mesh_id)?;

        let material = scene.material_of(entity);
        let model_matrix = scene.compute_world_matrix(entity.id);
        let entity_uniform = solid_entity_uniform(entity, material, model_matrix);
        let entity_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Entity Uniform"),
                contents: bytemuck::bytes_of(&entity_uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        // Upload the mesh's active bone palette: the live animated pose when a clip
        // is playing (#80), else the bind pose (#79). Skinned `"Asset"` meshes supply
        // a real palette computed from their imported skeleton; primitives and static
        // meshes leave an empty palette, so the GPU bones stay at identity and the
        // skinning shader is a no-op for them.
        let mut bones_data = *default_bones;
        for (i, bone) in mesh.active_palette().iter().take(64).enumerate() {
            bones_data.bones[i] = bone.to_cols_array();
        }
        let bones_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Bones Uniform"),
                contents: bytemuck::bytes_of(&bones_data),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let entity_bind_group =
            self.entity_bind_group("Entity Bind Group", &entity_buffer, &bones_buffer);

        // Bind Group 2 (Material): albedo + metallic + roughness + normal + emissive
        // maps, each resolved from the entity's material by path (default texture when
        // absent / not yet loaded), assembled against the expanded `material_layout`
        // (#202, #207). Order matches the layout's texture bindings (0,2,3,4,5).
        let maps = [
            self.resolve_map(material.and_then(|m| m.base_color_map.as_ref())),
            self.resolve_map(material.and_then(|m| m.metallic_map.as_ref())),
            self.resolve_map(material.and_then(|m| m.roughness_map.as_ref())),
            self.resolve_map(material.and_then(|m| m.normal_map.as_ref())),
            self.resolve_map(material.and_then(|m| m.emissive_map.as_ref())),
        ];
        let material_bind_group = self.material_bind_group(&maps);

        Some((
            entity.id,
            mesh_id,
            entity_buffer,
            bones_buffer,
            entity_bind_group,
            material_bind_group,
            gpu_mesh.num_indices,
        ))
    }

    /// Resolve a material map path to a resident GPU texture, falling back to the
    /// default texture when the path is absent or not yet uploaded.
    fn resolve_map(&self, path: Option<&String>) -> Rc<GpuTexture> {
        match path {
            Some(p) => self
                .gpu_textures
                .get(p)
                .cloned()
                .unwrap_or_else(|| Rc::clone(&self.default_texture)),
            None => Rc::clone(&self.default_texture),
        }
    }

    /// Build a group(2) material bind group from the five resolved map textures
    /// (albedo, metallic, roughness, normal, emissive — that order) + one shared
    /// sampler, against `material_layout`. Textures bind at 0,2,3,4,5; sampler at 1
    /// (binding 1 samples all five) (#202, #207).
    pub(super) fn material_bind_group(&self, maps: &[Rc<GpuTexture>; 5]) -> wgpu::BindGroup {
        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::Sampler(&self.default_texture.sampler),
        }];
        for (map, binding) in maps.iter().zip([0u32, 2, 3, 4, 5]) {
            entries.push(wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(&map.view),
            });
        }
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material Bind Group"),
            layout: &self.material_layout,
            entries: &entries,
        })
    }

    /// Create a group-1 bind group pairing an entity uniform buffer with a bones
    /// buffer against the shared `entity_bones_layout`.
    pub(super) fn entity_bind_group(
        &self,
        label: &str,
        entity_buf: &wgpu::Buffer,
        bones_buf: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &self.entity_bones_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: entity_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: bones_buf.as_entire_binding(),
                },
            ],
        })
    }
}

/// Compute the per-entity uniform (tint, lit flag, PBR params) for a solid mesh.
/// Tint is driven by components only — never by entity name. A game colours its
/// entities via its referenced material's `base_color`; the engine carries no
/// per-name colour assumptions. `material` is the entity's resolved library
/// material (`None` when it references none).
fn solid_entity_uniform(
    entity: &crate::components::Entity,
    material: Option<&MaterialAsset>,
    model_matrix: Mat4,
) -> EntityUniform {
    let is_lit = if entity.light.is_some() { 0u32 } else { 1u32 };

    let color_tint = if let Some(mat) = material {
        [mat.base_color[0], mat.base_color[1], mat.base_color[2], 1.0]
    } else if let Some(health) = &entity.health {
        if health.is_dead {
            [0.2, 0.2, 0.2, 1.0]
        } else {
            [1.0, 1.0, 1.0, 1.0]
        }
    } else {
        [1.0, 1.0, 1.0, 1.0]
    };

    let (metallic, roughness) = match material {
        Some(mat) => (mat.metallic, mat.roughness),
        None => (0.0, 0.5),
    };

    let use_texture = u32::from(material.is_some_and(|m| m.base_color_map.is_some()));
    let use_metallic_map = u32::from(material.is_some_and(|m| m.metallic_map.is_some()));
    let use_roughness_map = u32::from(material.is_some_and(|m| m.roughness_map.is_some()));
    let use_normal_map = u32::from(material.is_some_and(|m| m.normal_map.is_some()));
    let use_emissive_map = u32::from(material.is_some_and(|m| m.emissive_map.is_some()));

    // Flat emissive factor (#222), 4th lane unused. Defaults to black with no material.
    let emissive = match material {
        Some(mat) => [mat.emissive[0], mat.emissive[1], mat.emissive[2], 0.0],
        None => [0.0, 0.0, 0.0, 0.0],
    };

    EntityUniform {
        model_matrix: model_matrix.to_cols_array(),
        color_tint,
        use_texture,
        is_lit,
        metallic,
        roughness,
        use_metallic_map,
        use_roughness_map,
        use_normal_map,
        use_emissive_map,
        emissive,
    }
}
