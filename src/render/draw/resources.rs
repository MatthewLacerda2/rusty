//! Per-frame GPU resource pre-creation: shared resource-tuple type aliases, the
//! default bone uniform, solid entity resources, and the selection outline.
//! Editor overlays live in `draw_overlays`/`draw_path`. Extracted verbatim from
//! `Renderer::render` so each block returns owned resources that outlive the
//! render pass. Behavior unchanged.

use std::rc::Rc;

use crate::components::MaterialAsset;
use crate::render::{GpuTexture, MeshId, Renderer};
use crate::scene::Scene;

// One solid draw item: the entity id (its persistent entity + material bind groups
// live in `entity_pool`, keyed by this id — #210), the `MeshId` geometry key the
// pass resolves the shared vertex/index buffers through (#127), and the index count.
pub(crate) type SolidResource = (u32, MeshId, u32);
// A transparent draw item: the same draw tuple plus its view-space depth (distance
// along the camera forward to the entity origin). The transparent pass sorts on this
// key back-to-front so `ALPHA_BLENDING` composites correctly (#242).
pub(crate) type TransparentResource = (SolidResource, f32);

/// The solids split into the two passes a frame draws (#242). `opaque` (Opaque +
/// Cutout) rides the existing `draw_solids` path (REPLACE, depth write on); each
/// `transparent` item is deferred to the sorted alpha-blended pass after opaque.
/// Both share the same per-entity pool bind groups synced in `precreate_solid_resources`.
#[derive(Default)]
pub(crate) struct SolidResources {
    pub opaque: Vec<SolidResource>,
    pub transparent: Vec<TransparentResource>,
}
// The overlay resources keep only the buffers they own (the per-overlay entity
// uniform + any vertex buffer) plus their bind group; the bone palette they bind is
// the renderer's one shared identity buffer, so no per-overlay palette is allocated
// (#210).
pub(crate) type OutlineResource = (u32, MeshId, wgpu::Buffer, wgpu::BindGroup, u32);
pub(crate) type GridResource = (wgpu::Buffer, wgpu::BindGroup);
pub(crate) type AabbResource = (wgpu::Buffer, wgpu::Buffer, wgpu::BindGroup);
pub(crate) type AxisResource = (usize, wgpu::Buffer, wgpu::BindGroup);
pub(crate) type PathResource = (wgpu::Buffer, wgpu::Buffer, wgpu::BindGroup, u32);
// One probe gizmo line draw (#284): its world-space line-list vertex buffer, the flat
// overlay uniform it owns, its group-1 bind group, and the vertex count to draw.
pub(crate) type ProbeResource = (wgpu::Buffer, wgpu::Buffer, wgpu::BindGroup, u32);

/// The editor-only overlay resources for one scene pass (selection outline, grid,
/// collider AABBs, axis arrows). Empty outside editor mode. Bundled so the scene pass
/// and the per-camera loop pass a single value instead of four (#93).
#[derive(Default)]
pub(crate) struct Overlays {
    pub outline: Option<OutlineResource>,
    pub grid: Option<GridResource>,
    pub aabb: Vec<AabbResource>,
    pub axis: Vec<AxisResource>,
    // Light- and reflection-probe gizmos (#284): probe markers tinted by baked SH plus
    // reflection parallax-box wireframes. Editor-only, like the rest of `Overlays`.
    pub probes: Vec<ProbeResource>,
}

impl Renderer {
    /// The one shared identity bone palette buffer every overlay/non-skinned draw
    /// binds, so none of them allocate a per-draw 4 KB palette (#210).
    pub(crate) fn shared_bones_buffer(&self) -> &wgpu::Buffer {
        self.entity_pool
            .as_ref()
            .expect("entity pool present")
            .default_bones_buffer()
    }

    /// Pre-create the editor overlay resources for a pass; all-empty in play mode.
    pub(crate) fn precreate_overlays(&self, scene: &Scene, editor_mode: bool) -> Overlays {
        if !editor_mode {
            return Overlays::default();
        }
        Overlays {
            outline: self.precreate_outline(scene),
            grid: self.precreate_grid(),
            aabb: self.precreate_aabb(scene),
            axis: self.precreate_axis_arrows(scene),
            probes: self.precreate_probes(scene),
        }
    }

    pub(crate) fn precreate_solid_resources(
        &mut self,
        scene: &Scene,
        cam: &crate::render::Camera,
    ) -> SolidResources {
        let (cam_pos, cam_fwd) = (cam.position, cam.forward());
        let mut out = SolidResources::default();
        for entity in scene.iter() {
            if !entity.active {
                continue;
            }
            // During a static-cubemap capture, gather only static geometry — dynamic
            // actors must not bake into a probe/reflection (#243), mirroring the
            // shadow pass's `want_static` filter.
            if self.static_capture && !entity.is_static {
                continue;
            }
            // Skip meshes the active camera's culling mask excludes (#92).
            if !crate::scene::layer_in_mask(entity.layer, cam.culling_mask) {
                continue;
            }
            let Some((res, world_pos, transparent)) = self.sync_solid_resource(scene, &entity)
            else {
                continue;
            };
            if transparent {
                let depth = (world_pos - cam_pos).dot(cam_fwd);
                out.transparent.push((res, depth));
            } else {
                out.opaque.push(res);
            }
        }
        // Back-to-front: farthest (largest view-space depth) drawn first so nearer
        // translucent surfaces blend over what is behind them, draw order regardless.
        out.transparent.sort_by(|a, b| b.1.total_cmp(&a.1));
        out
    }

    /// Sync one solid entity's persistent pool slot (uniform + palette written in
    /// place, bind groups reused) and return its draw item, the entity's world-space
    /// origin (the transparent sort key's anchor), and whether its material is
    /// Transparent. `None` if its mesh is not resident on the GPU yet (#210).
    fn sync_solid_resource(
        &mut self,
        scene: &Scene,
        entity: &crate::components::Entity,
    ) -> Option<(SolidResource, glam::Vec3, bool)> {
        let mesh = entity.mesh.as_ref()?;
        let mesh_id = MeshId::from_mesh(mesh);
        let num_indices = self.gpu_meshes.get(&mesh_id)?.num_indices;

        let material = scene.material_of(entity);
        let transparent = material.is_some_and(MaterialAsset::is_transparent);
        let model_matrix = scene.compute_world_matrix(entity.id);
        let world_pos = model_matrix.w_axis.truncate();
        let uniform = crate::render::draw::uniforms::solid_entity_uniform(
            scene,
            entity,
            material,
            model_matrix,
        );
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

        let update = crate::render::gpu::entity_pool::SlotUpdate {
            uniform,
            palette: &palette,
            material_sig,
        };
        self.sync_entity_slot(entity.id, update, |s| {
            let maps = std::array::from_fn(|i| s.resolve_map(paths[i].as_ref()));
            s.material_bind_group(&maps)
        });

        Some(((entity.id, mesh_id, num_indices), world_pos, transparent))
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
    pub(crate) fn material_bind_group(&self, maps: &[Rc<GpuTexture>; 5]) -> wgpu::BindGroup {
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
    pub(crate) fn entity_bind_group(
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
