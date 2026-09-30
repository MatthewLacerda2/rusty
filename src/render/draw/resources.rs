//! Per-frame GPU resource pre-creation: the camera's solids as instanced draws (#470),
//! the material bind groups they share, and the overlay resource-tuple type aliases.
//! Editor overlays live in `overlays`/`axis`/`probes`. Each block returns owned
//! resources that outlive the render pass.

use std::rc::Rc;

use super::batch::{BatchKey, DrawItem, FrameDraws};
use crate::components::MaterialAsset;
use crate::render::gpu::draw_buffers::{group1, palette_uniform, FrameUpload};
use crate::render::{transform_aabb, Frustum, GpuTexture, MeshId, Renderer};
use crate::scene::Scene;

/// One camera's solids as instanced draws (#470), split into the two passes a frame
/// draws (#242): `draws.opaque` (Opaque + Cutout; REPLACE, depth write on) and
/// `draws.transparent` (back-to-front, alpha-blended). Both index the instance array
/// uploaded into `draw_buffers` by `precreate_solid_resources`.
#[derive(Default)]
pub(crate) struct SolidResources {
    pub draws: FrameDraws,
    /// Mesh entities the frustum cull skipped for this camera (#433's counters).
    pub culled: u32,
}
// The overlay resources keep only the buffers they own (the per-overlay entity
// uniform + any vertex buffer) plus their bind group; the bone palette they bind is
// the renderer's one shared identity buffer, so no per-overlay palette is allocated
// (#210).
pub(crate) type OutlineResource = (u32, MeshId, wgpu::Buffer, wgpu::BindGroup, u32);
pub(crate) type GridResource = (wgpu::Buffer, wgpu::BindGroup);
pub(crate) type AabbResource = (wgpu::Buffer, wgpu::Buffer, wgpu::BindGroup);
pub(crate) type AxisResource = (usize, wgpu::Buffer, wgpu::BindGroup);
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
    /// The one shared identity bone palette buffer every overlay binds, so none of
    /// them allocate a per-draw 4 KB palette (#210).
    pub(crate) fn shared_bones_buffer(&self) -> &wgpu::Buffer {
        self.draw_buffers.default_bones()
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

    /// Collect this camera's visible solids, batch repeated mesh + material pairs into
    /// instanced draws (#470), and upload the camera's packed draw data.
    pub(crate) fn precreate_solid_resources(
        &mut self,
        scene: &Scene,
        cam: &crate::scene::Camera,
        frustum: &Frustum,
    ) -> SolidResources {
        let (cam_pos, cam_fwd) = (cam.position, cam.forward());
        let (mut opaque, mut transparent, mut palettes) = (Vec::new(), Vec::new(), Vec::new());
        let mut culled = 0;
        for id in scene.world.ids_with_mesh() {
            if !scene.world.is_active(id) {
                continue;
            }
            // During a static-cubemap capture, gather only static geometry — dynamic
            // actors must not bake into a probe/reflection (#243), mirroring the
            // shadow pass's `want_static` filter.
            if self.static_capture && !scene.world.is_static(id) {
                continue;
            }
            // Skip meshes the active camera's culling mask excludes (#92).
            if !crate::scene::layer_in_mask(scene.world.layer(id), cam.culling_mask) {
                continue;
            }
            // View-frustum cull (#330): skip the uniform sync, binds, and draw for any
            // entity whose world-space AABB is fully outside what this camera can see.
            if self.is_culled(scene, id, frustum) {
                culled += 1;
                continue;
            }
            let Some((item, world_pos, is_transparent)) =
                self.solid_draw_item(scene, id, &mut palettes)
            else {
                continue;
            };
            if is_transparent {
                transparent.push((item, (world_pos - cam_pos).dot(cam_fwd)));
            } else {
                opaque.push(item);
            }
        }
        // Back-to-front: farthest (largest view-space depth) drawn first so nearer
        // translucent surfaces blend over what is behind them, draw order regardless.
        transparent.sort_by(|a, b| b.1.total_cmp(&a.1));
        let transparent = transparent.into_iter().map(|(item, _)| item).collect();
        let draws = FrameDraws::build(opaque, transparent, self.instancing);
        self.draw_buffers.upload(
            &self.device,
            &self.queue,
            &self.entity_bones_layout,
            FrameUpload {
                uniforms: &draws.uniforms(),
                palettes: &palettes,
                instances: &draws.instances,
            },
        );
        SolidResources { draws, culled }
    }

    /// Whether `entity` is fully outside `frustum` and can be skipped this pass (#330).
    /// Transforms the mesh's cached local AABB by the entity's world matrix (O(1)) and
    /// tests it. An entity with no mesh, or whose geometry is not resident on the GPU yet
    /// (no cached AABB), is never culled here — the downstream sync produces no draw for it
    /// anyway, and culling a not-yet-uploaded mesh could wrongly hide it on its first frame.
    fn is_culled(&self, scene: &Scene, id: u32, frustum: &Frustum) -> bool {
        let Some(mesh) = scene.world.mesh(id) else {
            return false;
        };
        // Skinned meshes are never culled here — their AABB is the rest pose (#330).
        if mesh.is_skinned() {
            return false;
        }
        let Some(gpu_mesh) = self.gpu_meshes.get(&MeshId::from_mesh(&mesh)) else {
            return false;
        };
        let (min, max) = transform_aabb(
            gpu_mesh.local_aabb.0,
            gpu_mesh.local_aabb.1,
            scene.world_matrix(id),
        );
        !frustum.intersects_aabb(min, max)
    }

    /// One solid's draw item: its batch key (mesh, material group, bone slot, per-draw
    /// uniform) and instance, plus its world-space origin (the transparent sort key's
    /// anchor) and whether its material is Transparent. A skinned mesh's palette is
    /// appended to `palettes`, giving it a bone slot — and so a draw — of its own.
    /// `None` if its mesh is not resident on the GPU yet.
    fn solid_draw_item(
        &mut self,
        scene: &Scene,
        id: u32,
        palettes: &mut Vec<crate::render::BoneUniform>,
    ) -> Option<(DrawItem, glam::Vec3, bool)> {
        let mesh = scene.world.mesh(id)?;
        let mesh_id = MeshId::from_mesh(&mesh);
        let num_indices = self.gpu_meshes.get(&mesh_id)?.num_indices;

        let material = scene.material_asset_of(id);
        let transparent = material.is_some_and(MaterialAsset::is_transparent);
        let model_matrix = scene.world_matrix(id);
        let uniform = super::uniforms::solid_entity_uniform(scene, id, material);
        let instance =
            super::uniforms::solid_instance(scene, id, model_matrix, self.capture_probe_bounce);

        // The active bone palette: the live animated pose when a clip plays (#80),
        // else the bind pose (#79). Primitives/static meshes leave it empty and bind
        // the shared identity palette in slot 0.
        let palette = mesh.active_palette();
        let bones = if palette.is_empty() {
            0
        } else {
            palettes.push(palette_uniform(palette));
            palettes.len() as u32
        };

        let key = BatchKey {
            mesh: mesh_id,
            material: self.material_index(material),
            bones,
            uniform: uniform.words(),
        };
        let item = DrawItem {
            key,
            num_indices,
            instance,
        };
        Some((item, model_matrix.w_axis.truncate(), transparent))
    }

    /// The material cache index for `material`'s five maps (albedo, metallic,
    /// roughness, normal, emissive), building the bind group on first use. The key is
    /// the *resolved* signature, so a late-loaded texture gets a fresh group (#207).
    fn material_index(&mut self, material: Option<&MaterialAsset>) -> usize {
        let paths = [
            material.and_then(|m| m.base_color_map.clone()),
            material.and_then(|m| m.metallic_map.clone()),
            material.and_then(|m| m.roughness_map.clone()),
            material.and_then(|m| m.normal_map.clone()),
            material.and_then(|m| m.emissive_map.clone()),
        ];
        let sig = std::array::from_fn(|i| self.resolved_key(paths[i].as_ref()));
        if let Some(i) = self.materials.lookup(&sig) {
            return i;
        }
        let maps = std::array::from_fn(|i| self.resolve_map(paths[i].as_ref()));
        let group = self.material_bind_group(&maps);
        self.materials.insert(sig, group)
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

    /// Create an overlay's group-1 bind group: its own uniform buffer, a bones buffer,
    /// and the shared one-element identity instance array — an overlay is one draw of
    /// instance 0, its transform carried in the uniform. Bound at offsets `[0, 0]`.
    pub(crate) fn entity_bind_group(
        &self,
        label: &str,
        entity_buf: &wgpu::Buffer,
        bones_buf: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        group1(
            &self.device,
            &self.entity_bones_layout,
            label,
            [entity_buf, bones_buf, self.draw_buffers.identity_instance()],
        )
    }

    /// Distinct material bind groups built so far — tests prove they are shared and
    /// do not grow per frame.
    #[cfg(test)]
    pub(crate) fn material_group_count(&self) -> usize {
        self.materials.len()
    }
}
