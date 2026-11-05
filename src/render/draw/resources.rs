//! Per-frame GPU resource pre-creation: the camera's solids (and mesh particles, #440)
//! as instanced draws (#470), and the overlay resource-tuple type aliases. The
//! material bind groups they share live in `materials`; editor overlays in
//! `overlays` and its children. Each block returns owned resources that outlive the
//! render pass.

use super::batch::sort::back_to_front;
use super::batch::{DrawItem, FrameDraws};
use super::overlays::{OverlayBindings, OverlayDraws};
use super::visibility::{CameraCulls, ViewCull};
use crate::render::gpu::draw_buffers::{FrameUpload, JointMatrix};
use crate::render::lod::LodSelection;
use crate::render::motion::MotionHistory;
use crate::render::passes::outline::{outlined, OutlineBounds};
use crate::render::scene::CameraSolids;
use crate::render::{Frustum, Renderer};
use crate::scene::Scene;

/// One camera's solids as instanced draws (#470), split into the two passes a frame
/// draws (#242): `draws.opaque` (Opaque + Cutout; REPLACE, depth write on) and
/// `draws.transparent` (back-to-front, alpha-blended). Both index the instance array
/// uploaded into `draw_buffers` by `precreate_solid_resources`.
#[derive(Default)]
pub(crate) struct SolidResources {
    pub draws: FrameDraws,
    /// What this camera's culling skipped, and how (#433, #472, #1714).
    pub culls: CameraCulls,
    /// Mesh entities drawn: one each, however many material runs it draws in (#1285).
    pub visible: u32,
    /// Mesh particles among the instances (#440) — counted as particles, not entities.
    pub mesh_particles: u32,
    /// Where the solids the selection outline rings sit in world space (#1175);
    /// `None` when it rings none.
    pub outline_bounds: Option<OutlineBounds>,
}
// Each overlay keeps only what it owns (any vertex buffer) and its slot of the pass's
// one overlay group 1 ([`OverlayBindings`], #1616): its uniform there places it and
// names its tint's record, and the joints and instance it binds are the renderer's
// shared identities (#210). The selection outline is not an overlay: it is its own
// pass over the solids' own draws (`passes::outline`, #1175).
pub(crate) type GridResource = u32;
pub(crate) type AabbResource = (wgpu::Buffer, u32);
pub(crate) type AxisResource = (usize, u32);
// One probe gizmo line draw (#284): its world-space line-list vertex buffer, its
// slot, and the vertex count to draw.
pub(crate) type ProbeResource = (wgpu::Buffer, u32, u32);

/// The editor-only overlay resources for one handles pass (grid, collider AABBs,
/// axis arrows, probe gizmos), drawn after post-FX (#1224). Bundled so the pass
/// takes a single value instead of four (#93).
#[derive(Default)]
pub(crate) struct Overlays {
    /// The group 1 every overlay binds at its slot; `None` when there is none.
    pub bindings: Option<OverlayBindings>,
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
        let mut draws = OverlayDraws::default();
        let grid = self.precreate_grid(&mut draws);
        let mut aabb = self.precreate_aabb(scene, &mut draws);
        aabb.extend(self.precreate_occlusion(scene, &mut draws));
        let axis = self.precreate_axis_arrows(scene, &mut draws);
        let probes = self.precreate_probes(scene, &mut draws);
        Overlays {
            bindings: self.overlay_bindings(&draws),
            grid,
            aabb,
            axis,
            probes,
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
        (outline, history): (bool, Option<&mut MotionHistory>),
    ) -> SolidResources {
        let eye = (cam.position, cam.forward());
        let (mut opaque, mut transparent, mut joints) = (Vec::new(), Vec::new(), Vec::new());
        let limits = self.device.limits();
        let binding = limits.max_storage_buffer_binding_size;
        let mut proxies = self
            .proxy_volumes
            .frame(binding.min(limits.max_buffer_size));
        let ringed = if outline {
            outlined(scene, lod)
        } else {
            Vec::new()
        };
        let view = ViewCull {
            mask: cam.culling_mask,
            lod,
            frustum,
            small: self.small_object_cull(cam),
            occlusion: self.occlusion_eye(cam),
        };
        let out = CameraSolids {
            opaque: &mut opaque,
            transparent: &mut transparent,
            joints: &mut joints,
            proxies: &mut proxies,
        };
        let (culls, visible) = self.camera_solids(scene, (&view, eye, &ringed), out, history);
        let mesh_particles = self.push_mesh_particles(scene, cam, (&mut opaque, &mut transparent));
        let outline_bounds = self.outline_bounds(scene, &ringed);
        SolidResources {
            draws: self.upload_solids(opaque, transparent, (&joints, &proxies.data)),
            culls,
            visible,
            mesh_particles,
            outline_bounds,
        }
    }

    /// Sort the translucent items back to front, batch both lists into instanced
    /// draws (#470) and upload the camera's packed draw data.
    fn upload_solids(
        &mut self,
        opaque: Vec<DrawItem>,
        mut transparent: Vec<(DrawItem, f32)>,
        (joints, proxies): (&[JointMatrix], &[[f32; 4]]),
    ) -> FrameDraws {
        back_to_front(&mut transparent);
        let transparent = transparent.into_iter().map(|(item, _)| item).collect();
        let draws = FrameDraws::build(opaque, transparent, self.instancing);
        let (uniforms, materials) = draws.pack();
        self.draw_buffers.upload(
            &self.device,
            &self.queue,
            &self.entity_bones_layout,
            FrameUpload {
                uniforms: &uniforms,
                joints,
                instances: &draws.instances,
                materials: &materials,
                proxies,
                objects: &mut self.retained.objects,
            },
        );
        self.retained.objects.end_camera();
        self.frame_counters.render_upload_bytes += self.draw_buffers.take_written();
        draws
    }
}
