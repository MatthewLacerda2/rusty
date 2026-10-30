//! Per-frame GPU resource pre-creation: the camera's solids (and mesh particles, #440)
//! as instanced draws (#470), and the overlay resource-tuple type aliases. The
//! material bind groups they share live in `materials`; editor overlays in
//! `overlays` and its children. Each block returns owned resources that outlive the
//! render pass.

use super::batch::sort::{back_to_front, view_depth};
use super::batch::{BatchKey, DrawItem, FrameDraws};
use super::materials::entity_params;
use crate::components::MaterialAsset;
use crate::render::gpu::draw_buffers::{group1, push_palette, FrameUpload, JointMatrix};
use crate::render::lod::LodSelection;
use crate::render::motion::{track, MotionHistory};
use crate::render::passes::outline::{outlined, OutlineBounds};
use crate::render::{Frustum, MeshId, Renderer};
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
    /// Mesh entities hidden because their LOD level is not the one shown (#472).
    pub lod_hidden: u32,
    /// Mesh entities drawn: one each, however many material runs it draws in (#1285).
    pub visible: u32,
    /// Mesh particles among the instances (#440) — counted as particles, not entities.
    pub mesh_particles: u32,
    /// Where the solids the selection outline rings sit in world space (#1175);
    /// `None` when it rings none.
    pub outline_bounds: Option<OutlineBounds>,
}
// The overlay resources keep only the buffers they own (the per-overlay entity
// uniform + any vertex buffer) plus their bind group; the joint array they bind is
// the renderer's one shared identity matrix, so no per-overlay palette is allocated
// (#210). The selection outline is not an overlay: it is its own pass over the
// solids' own draws (`passes::outline`, #1175).
pub(crate) type GridResource = (wgpu::Buffer, wgpu::BindGroup);
pub(crate) type AabbResource = (wgpu::Buffer, wgpu::Buffer, wgpu::BindGroup);
pub(crate) type AxisResource = (usize, wgpu::Buffer, wgpu::BindGroup);
// One probe gizmo line draw (#284): its world-space line-list vertex buffer, the flat
// overlay uniform it owns, its group-1 bind group, and the vertex count to draw.
pub(crate) type ProbeResource = (wgpu::Buffer, wgpu::Buffer, wgpu::BindGroup, u32);

/// The editor-only overlay resources for one handles pass (grid, collider AABBs,
/// axis arrows, probe gizmos), drawn after post-FX (#1224). Bundled so the pass
/// takes a single value instead of four (#93).
#[derive(Default)]
pub(crate) struct Overlays {
    pub grid: Option<GridResource>,
    pub aabb: Vec<AabbResource>,
    pub axis: Vec<AxisResource>,
    // Light- and reflection-probe gizmos (#284): probe markers tinted by baked SH plus
    // reflection parallax-box wireframes. Editor-only, like the rest of `Overlays`.
    pub probes: Vec<ProbeResource>,
}

impl Renderer {
    /// The one shared identity joint buffer the overlays bind, so none of them
    /// allocate a per-draw palette (#210).
    pub(crate) fn shared_bones_buffer(&self) -> &wgpu::Buffer {
        self.draw_buffers.default_bones()
    }

    /// Pre-create the editor overlay resources for one handles pass. Only an editor
    /// camera draws one (`draw::camera`'s `handles` stage).
    pub(crate) fn precreate_overlays(&self, scene: &Scene) -> Overlays {
        Overlays {
            grid: self.precreate_grid(),
            aabb: self.precreate_aabb(scene),
            axis: self.precreate_axis_arrows(scene),
            probes: self.precreate_probes(scene),
        }
    }

    /// Collect this camera's visible solids (skipping the LOD levels `lod` hides), batch repeated mesh + material pairs into
    /// instanced draws (#470), and upload the camera's packed draw data. `marks`:
    /// with `outline` (an editor camera), the solids the selection outline rings are
    /// marked, so it draws from their draws (#1175); with `history` (a view writing
    /// motion vectors, #1435), each solid also carries its last-frame pose.
    pub(crate) fn precreate_solid_resources(
        &mut self,
        scene: &Scene,
        cam: &crate::scene::Camera,
        frustum: &Frustum,
        lod: &LodSelection,
        (outline, mut history): (bool, Option<&mut MotionHistory>),
    ) -> SolidResources {
        let (cam_pos, cam_fwd) = (cam.position, cam.forward());
        let (mut opaque, mut transparent, mut joints) = (Vec::new(), Vec::new(), Vec::new());
        let (mut culled, mut lod_hidden, mut visible) = (0, 0, 0);
        let ringed = if outline {
            outlined(scene, lod)
        } else {
            Vec::new()
        };
        for id in scene.world.ids_with_mesh() {
            if !self.camera_draws(scene, id, cam.culling_mask) {
                continue;
            }
            // A level of detail this camera does not show (#472).
            if lod.hides(id) {
                lod_hidden += 1;
                continue;
            }
            // View-frustum cull (#330): skip the uniform sync, binds, and draw for any
            // entity whose world-space AABB is fully outside what this camera can see.
            let bounds = self.world_bounds(scene, id);
            if bounds.is_some_and(|(min, max)| !frustum.intersects_aabb(min, max)) {
                culled += 1;
                continue;
            }
            let motion = history.as_deref_mut();
            let eye = (cam_pos, cam_fwd);
            let items = self.solid_draw_items(scene, id, (bounds, eye), &mut joints, motion);
            visible += u32::from(!items.is_empty());
            for (mut item, world_pos, is_transparent) in items {
                item.outline = ringed.contains(&id);
                if is_transparent {
                    transparent.push((item, view_depth(world_pos, cam_pos, cam_fwd)));
                } else {
                    opaque.push(item);
                }
            }
        }
        let mesh_particles = self.push_mesh_particles(scene, cam, (&mut opaque, &mut transparent));
        let outline_bounds = self.outline_bounds(scene, &ringed);
        SolidResources {
            draws: self.upload_solids(opaque, transparent, &joints),
            culled,
            lod_hidden,
            visible,
            mesh_particles,
            outline_bounds,
        }
    }

    /// Whether a camera whose culling mask (#92) is `mask` draws mesh entity `id` at
    /// all, before LOD and frustum culling.
    fn camera_draws(&self, scene: &Scene, id: u32, mask: u32) -> bool {
        // An inactive mesh, or a shadows-only one: it casts, no camera draws it (#1571).
        let mode = scene.world.mesh(id).map(|m| m.cast_shadows);
        if !scene.world.is_active_in_hierarchy(id) || mode.is_some_and(|m| !m.is_visible()) {
            return false;
        }
        // During a static-cubemap capture, gather only static geometry — dynamic
        // actors must not bake into a probe/reflection (#243), mirroring the
        // shadow pass's `want_static` filter.
        if self.capture.static_only && !scene.world.is_static(id) {
            return false;
        }
        crate::scene::layer_in_mask(scene.world.layer(id), mask)
    }

    /// Sort the translucent items back to front, batch both lists into instanced
    /// draws (#470) and upload the camera's packed draw data.
    fn upload_solids(
        &mut self,
        opaque: Vec<DrawItem>,
        mut transparent: Vec<(DrawItem, f32)>,
        joints: &[JointMatrix],
    ) -> FrameDraws {
        back_to_front(&mut transparent);
        let transparent = transparent.into_iter().map(|(item, _)| item).collect();
        let draws = FrameDraws::build(opaque, transparent, self.instancing);
        self.draw_buffers.upload(
            &self.device,
            &self.queue,
            &self.entity_bones_layout,
            FrameUpload {
                uniforms: &draws.uniforms(),
                joints,
                instances: &draws.instances,
            },
        );
        draws
    }

    /// The entity's world bounds `(min, max)`: its mesh's cached local AABB under the
    /// entity's world matrix, or its animated bound when skinned (#833). The frustum
    /// cull (#330) tests them and the draw picks its reflection probe at their centre
    /// (#1206). `None` for an entity with no mesh, whose geometry is not resident on
    /// the GPU yet, or whose pose cannot be bounded: it is never culled, since the
    /// downstream sync produces no draw for a non-resident mesh anyway, and culling a
    /// not-yet-uploaded mesh could wrongly hide it on its first frame.
    fn world_bounds(&self, scene: &Scene, id: u32) -> Option<(glam::Vec3, glam::Vec3)> {
        let mesh = scene.world.mesh(id)?;
        let gpu_mesh = self.gpu_meshes.get(&MeshId::from_mesh(&mesh))?;
        gpu_mesh.world_bounds(&mesh, scene.world.local_to_world(id))
    }

    /// One solid's draw items, one per material slot (#1285), each drawing its index
    /// run with that slot's material ([`GpuMesh::runs`](crate::render::GpuMesh::runs)):
    /// its batch key (mesh, run, material group, per-draw uniform) and instance, plus its world-space origin (the transparent sort key's
    /// anchor) and whether its material is Transparent. A skinned mesh's palette is
    /// appended to `joints` once, and its uniform's `bone_base` points at it — so it is
    /// a draw of its own. `bounds` are its world bounds when known
    /// ([`Self::world_bounds`]); its reflection probe is picked at their centre, and
    /// its view depth, from the camera `eye` (position, forward), is taken there.
    /// With `history`, its instance carries its last-frame pose (#1435).
    /// Empty if its mesh is not resident on the GPU yet.
    fn solid_draw_items(
        &mut self,
        scene: &Scene,
        id: u32,
        (bounds, eye): (Option<(glam::Vec3, glam::Vec3)>, (glam::Vec3, glam::Vec3)),
        joints: &mut Vec<JointMatrix>,
        history: Option<&mut MotionHistory>,
    ) -> Vec<(DrawItem, glam::Vec3, bool)> {
        let Some(mesh) = scene.world.mesh(id) else {
            return Vec::new();
        };
        let mesh_id = MeshId::from_mesh(&mesh);
        let Some(gpu_mesh) = self.gpu_meshes.get(&mesh_id) else {
            return Vec::new();
        };
        let runs = gpu_mesh.runs(&mesh, scene.world.material(id).as_deref());
        let model_matrix = scene.world.local_to_world(id);
        let origin = model_matrix.w_axis.truncate();
        let mut instance = super::uniforms::solid_instance(
            scene,
            id,
            model_matrix,
            self.capture.probe_bounce,
            &self.lightmaps,
        );
        // Like Unity, the reflection probe is picked per object, at the centre of its
        // world bounds (its origin when they are unknown), #1206, and the opaque pass
        // orders it by that centre's view depth (#1499).
        let centre = bounds.map_or(origin, |(min, max)| (min + max) * 0.5);
        instance.reflection_probe = self.reflection_probes.probe_for(centre);

        // The active bone palette: the live animated pose when a clip plays (#80),
        // else the bind pose (#79). Primitives/static meshes leave it empty and bind
        // the shared identity joint at element 0. The base lives in the uniform, so
        // two skinned entities never share a batch.
        let palette = mesh.active_palette();
        let bone_base = push_palette(joints, palette);
        if let Some(history) = history.filter(|_| has_opaque_run(scene, id, &runs)) {
            let pose = (model_matrix, palette);
            track(history, id, pose, bone_base, joints, &mut instance);
        }
        let mut items = Vec::with_capacity(runs.len());
        for (slot, indices) in runs {
            let entry = scene.material_entry_at(id, slot);
            let material = entry.map(|(_, asset)| asset);
            let transparent = material.is_some_and(MaterialAsset::is_transparent);
            let mut uniform = self.solid_uniform(scene, id, material);
            uniform.bone_base = bone_base;
            let shader = material.and_then(|m| m.shader.as_deref());
            let pipeline = self.surface_shaders.pipeline_id(&self.device, shader);
            let key = BatchKey {
                pipeline,
                mesh: mesh_id.clone(),
                indices: indices.into(),
                material: self.material_index(entry, pipeline, entity_params(scene, id, slot)),
                uniform: uniform.words(),
            };
            let item = DrawItem {
                key,
                instance,
                depth: view_depth(centre, eye.0, eye.1),
                outline: false,
            };
            items.push((item, origin, transparent));
        }
        items
    }

    /// Create an overlay's group-1 bind group: its own uniform buffer, a bones buffer,
    /// and the shared one-element identity instance array — an overlay is one draw of
    /// instance 0, its transform carried in the uniform. Bound at offset `[0]`.
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
}

/// Whether any of entity `id`'s material runs draws in the opaque pass: only those
/// write velocity, so an all-transparent entity keeps no motion pose (#1435).
fn has_opaque_run(scene: &Scene, id: u32, runs: &[(usize, std::ops::Range<u32>)]) -> bool {
    runs.iter().any(|&(slot, _)| {
        !scene
            .material_entry_at(id, slot)
            .is_some_and(|(_, m)| m.is_transparent())
    })
}
