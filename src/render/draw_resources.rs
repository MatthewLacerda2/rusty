//! Per-frame GPU resource pre-creation: shared resource-tuple type aliases, the
//! default bone uniform, solid entity resources, and the selection outline.
//! Editor overlays live in `draw_overlays`/`draw_path`. Extracted verbatim from
//! `Renderer::render` so each block returns owned resources that outlive the
//! render pass. Behavior unchanged.

use glam::Mat4;
use std::rc::Rc;

use super::{EntityUniform, GpuTexture, MeshId, Renderer};
use crate::components::MaterialAsset;
use crate::scene::Scene;

// One solid draw item: the entity id (its persistent entity + material bind groups
// live in `entity_pool`, keyed by this id — #210), the `MeshId` geometry key the
// pass resolves the shared vertex/index buffers through (#127), and the index count.
pub(super) type SolidResource = (u32, MeshId, u32);
// The overlay resources keep only the buffers they own (the per-overlay entity
// uniform + any vertex buffer) plus their bind group; the bone palette they bind is
// the renderer's one shared identity buffer, so no per-overlay palette is allocated
// (#210).
pub(super) type OutlineResource = (u32, MeshId, wgpu::Buffer, wgpu::BindGroup, u32);
pub(super) type GridResource = (wgpu::Buffer, wgpu::BindGroup);
pub(super) type AabbResource = (wgpu::Buffer, wgpu::Buffer, wgpu::BindGroup);
pub(super) type AxisResource = (usize, wgpu::Buffer, wgpu::BindGroup);
pub(super) type PathResource = (wgpu::Buffer, wgpu::Buffer, wgpu::BindGroup, u32);

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
    /// The one shared identity bone palette buffer every overlay/non-skinned draw
    /// binds, so none of them allocate a per-draw 4 KB palette (#210).
    pub(super) fn shared_bones_buffer(&self) -> &wgpu::Buffer {
        self.entity_pool
            .as_ref()
            .expect("entity pool present")
            .default_bones_buffer()
    }

    /// Pre-create the editor overlay resources for a pass; all-empty in play mode.
    pub(super) fn precreate_overlays(&self, scene: &Scene, editor_mode: bool) -> Overlays {
        if !editor_mode {
            return Overlays::default();
        }
        Overlays {
            outline: self.precreate_outline(scene),
            grid: self.precreate_grid(),
            aabb: self.precreate_aabb(scene),
            axis: self.precreate_axis_arrows(scene),
        }
    }

    pub(super) fn precreate_solid_resources(
        &mut self,
        scene: &Scene,
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
            if let Some(res) = self.sync_solid_resource(scene, &entity) {
                solid_render_resources.push(res);
            }
        }
        solid_render_resources
    }

    /// Sync one solid entity's persistent pool slot (uniform + palette written in
    /// place, bind groups reused) and return its draw item, or `None` if its mesh is
    /// not resident on the GPU yet (#210).
    fn sync_solid_resource(
        &mut self,
        scene: &Scene,
        entity: &crate::components::Entity,
    ) -> Option<SolidResource> {
        let mesh = entity.mesh.as_ref()?;
        let mesh_id = MeshId::from_mesh(mesh);
        let num_indices = self.gpu_meshes.get(&mesh_id)?.num_indices;

        let material = scene.material_of(entity);
        let model_matrix = scene.compute_world_matrix(entity.id);
        let uniform = solid_entity_uniform(scene, entity, material, model_matrix);
        // The active bone palette: the live animated pose when a clip plays (#80),
        // else the bind pose (#79). Primitives/static meshes leave it empty, so the
        // pool binds the shared identity palette and allocates no per-entity buffer.
        let palette = mesh.active_palette().to_vec();

        // Five material map paths (albedo, metallic, roughness, normal, emissive).
        // The signature is the *resolved* key (the path only when resident, else
        // empty for the default), so the material bind group is rebuilt exactly when
        // a map's resolved texture changes — including a late-loaded texture (#207).
        let paths = [
            material.and_then(|m| m.base_color_map.clone()),
            material.and_then(|m| m.metallic_map.clone()),
            material.and_then(|m| m.roughness_map.clone()),
            material.and_then(|m| m.normal_map.clone()),
            material.and_then(|m| m.emissive_map.clone()),
        ];
        let material_sig = std::array::from_fn(|i| self.resolved_key(paths[i].as_ref()));

        let update = super::entity_pool::SlotUpdate {
            uniform,
            palette: &palette,
            material_sig,
        };
        self.sync_entity_slot(entity.id, update, |s| {
            let maps = std::array::from_fn(|i| s.resolve_map(paths[i].as_ref()));
            s.material_bind_group(&maps)
        });

        Some((entity.id, mesh_id, num_indices))
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

    /// The cache key a map path resolves to: the path when its texture is resident,
    /// else empty (the default texture). Lets the pool detect a late-loaded map.
    fn resolved_key(&self, path: Option<&String>) -> String {
        match path {
            Some(p) if self.gpu_textures.contains_key(p) => p.clone(),
            _ => String::new(),
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
    scene: &Scene,
    entity: &crate::components::Entity,
    material: Option<&MaterialAsset>,
    model_matrix: Mat4,
) -> EntityUniform {
    let is_lit = if entity.light.is_some() { 0u32 } else { 1u32 };
    let (use_sh, sh) = entity_probe_sh(scene, entity, model_matrix);

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
        use_sh,
        _sh_pad: [0; 3],
        sh,
    }
}

/// Resolve the light-probe SH for one entity (#240): the scene's probe field sampled
/// (trilinear) at the entity's world position, flattened to the `vec4`-padded GPU
/// layout. Only NON-STATIC objects opt in — static geometry keeps the flat ambient
/// term (and is the bake's target, not its consumer). Returns `(use_sh, coeffs)`;
/// `use_sh == 0` with zeroed coeffs when the entity is static or no probe covers it.
fn entity_probe_sh(
    scene: &Scene,
    entity: &crate::components::Entity,
    model_matrix: Mat4,
) -> (u32, [[f32; 4]; 9]) {
    if entity.is_static {
        return (0, [[0.0; 4]; 9]);
    }
    let position = model_matrix.w_axis.truncate();
    match scene.probes.sample(position) {
        Some(probe) => {
            let mut sh = [[0.0f32; 4]; 9];
            for (i, c) in probe.coeffs.iter().enumerate() {
                sh[i] = [c[0], c[1], c[2], 0.0];
            }
            (1, sh)
        }
        None => (0, [[0.0; 4]; 9]),
    }
}
