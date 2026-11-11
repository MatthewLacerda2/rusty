//! Per-frame GPU resource pre-creation: the camera's solids (and mesh particles, #440)
//! as instanced draws (#470), and the overlay resource-tuple type aliases. The
//! material bind groups they share live in `materials`; editor overlays in
//! `overlays` and its children. Each block returns owned resources that outlive the
//! render pass.

use super::batch::sort::{transparent_order, TransparentOrder};
use super::batch::{DrawItem, FrameDraws};
use super::overlays::{BallResource, ColliderScope, OverlayBindings, OverlayDraws};
use super::visibility::{CameraCulls, ViewCull};
use crate::core::application::{Gizmo, SceneGizmos};
use crate::render::gpu::draw_buffers::{FrameUpload, JointMatrix};
use crate::render::lod::LodSelection;
use crate::render::motion::MotionHistory;
use crate::render::passes::outline::{outlined, OutlineBounds};
use crate::render::scene::CameraSolids;
use crate::render::{Frustum, Renderer};
use crate::scene::Scene;
use glam::Vec3;

/// One camera's solids as instanced draws (#470), split into the two passes a frame
/// draws (#242): `draws.opaque` (Opaque + Cutout; REPLACE, depth write on) and
/// `draws.transparent` (by sorting priority, then back-to-front; alpha-blended). Both index the instance array
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
    /// The objects GPU occlusion culling may test, by slot, with their world bounds
    /// (#1963; empty when the camera does not cull by Hi-Z), and, once its stage
    /// ran, the opaque draws it decided.
    pub tested: Vec<(u32, (Vec3, Vec3))>,
    pub culled: Option<crate::render::passes::hiz::Culled>,
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

/// The editor-only overlay resources for one handles pass (grid, occlusion boxes,
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
    /// The reflection probes' mirror balls (#1911).
    pub balls: Vec<BallResource>,
    /// The navmesh gizmo's slot (#1910); its vertex buffer is the renderer's, kept.
    pub navmesh: Option<u32>,
    /// The collider and `Debug.Draw*` line gizmos (#1882): one draw, coloured per
    /// vertex, through the vertex-colour line pipeline.
    pub wires: Option<ProbeResource>,
}

impl Renderer {
    /// The one shared identity joint buffer the overlays bind, so none of them
    /// allocate a per-draw palette (#210).
    pub(crate) fn shared_bones_buffer(&self) -> &wgpu::Buffer {
        self.draw_buffers.default_bones()
    }

    /// Pre-create the editor overlay resources for one handles pass. Only an editor
    /// camera draws one (`draw::camera`'s `handles` stage).
    /// `gizmos` is the Scene view's set (#1909): what it leaves off is not built.
    /// `eye` orders the colliders' wires nearest first (#1882).
    pub(crate) fn precreate_overlays(
        &self,
        scene: &Scene,
        gizmos: &SceneGizmos,
        eye: Vec3,
    ) -> Overlays {
        let mut draws = OverlayDraws::default();
        let grid = self.precreate_grid(&mut draws);
        let aabb = self.precreate_occlusion(scene, &mut draws);
        let axis = self.precreate_axis_arrows(scene, &mut draws);
        let probes = self.precreate_probes(scene, gizmos, &mut draws);
        let balls = self.precreate_probe_balls(scene, gizmos, &mut draws);
        let navmesh = self.precreate_navmesh(gizmos, &mut draws);
        let scope = match gizmos.on(Gizmo::Colliders) {
            true => ColliderScope::All,
            false => ColliderScope::Selected,
        };
        let wires = self.precreate_wires(scene, (scope, eye), &mut draws);
        Overlays {
            bindings: self.overlay_bindings(&draws),
            grid,
            aabb,
            axis,
            probes,
            balls,
            navmesh,
            wires,
        }
    }

    /// A game frame's gizmos (#1882): every collider and the debug lines, nothing
    /// of the editor's own (no grid, arrows or probes).
    pub(crate) fn precreate_game_overlays(&self, scene: &Scene, eye: Vec3) -> Overlays {
        let mut draws = OverlayDraws::default();
        let wires = self.precreate_wires(scene, (ColliderScope::All, eye), &mut draws);
        Overlays {
            bindings: self.overlay_bindings(&draws),
            wires,
            ..Default::default()
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
        // Only a camera that culls by Hi-Z reads what it may test (#1963).
        let mut tested = Vec::new();
        let hiz = self.hiz_culls(cam);
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
            tested: hiz.then_some(&mut tested),
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
            tested,
            culled: None,
        }
    }

    /// Sort the translucent items by priority, then back to front (#2039), batch both lists into instanced
    /// draws (#470) and upload the camera's packed draw data.
    fn upload_solids(
        &mut self,
        opaque: Vec<DrawItem>,
        mut transparent: Vec<(DrawItem, TransparentOrder)>,
        (joints, proxies): (&[JointMatrix], &[[f32; 4]]),
    ) -> FrameDraws {
        transparent_order(&mut transparent);
        let transparent = transparent.into_iter().map(|(item, _)| item).collect();
        let records = &self.retained.records;
        let draws = FrameDraws::build((opaque, transparent), records, self.instancing);
        let uniforms = draws.pack();
        self.draw_buffers.upload(
            &self.device,
            &self.queue,
            &self.entity_bones_layout,
            FrameUpload {
                uniforms: &uniforms,
                joints,
                instances: &draws.instances,
                proxies,
                objects: &mut self.retained.objects,
                materials: self.retained.records.table_for_sync(),
            },
        );
        self.retained.objects.end_camera();
        self.frame_counters.render_upload_bytes += self.draw_buffers.take_written();
        draws
    }
}
