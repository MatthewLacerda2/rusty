use super::batch::sort::{transparent_order, TransparentOrder};
use super::batch::{DrawItem, FrameDraws, OpaqueOrder};
use super::overlays::{BallResource, ColliderScope, OverlayBindings, OverlayDraws};
use super::visibility::{CameraCulls, ViewCull};
use crate::gpu::draw_buffers::{FrameUpload, JointMatrix};
use crate::lod::LodSelection;
use crate::motion::MotionHistory;
use crate::passes::outline::{outlined, OutlineBounds};
use crate::scene::CameraSolids;
use crate::{Frustum, Renderer};
use glam::Vec3;
use rusty_core::application::{Gizmo, SceneGizmos};
use rusty_scene::Scene;

#[derive(Default)]
pub(crate) struct SolidResources {
    pub draws: FrameDraws,
    pub culls: CameraCulls,
    pub visible: u32,
    pub mesh_particles: u32,
    pub outline_bounds: Option<OutlineBounds>,
    pub tested: Vec<(u32, (Vec3, Vec3))>,
    pub culled: Option<crate::passes::hiz::Culled>,
    pub floored: bool,
}
pub(crate) type GridResource = u32;
pub(crate) type AabbResource = (wgpu::Buffer, u32);
pub(crate) type AxisResource = (usize, u32);
pub(crate) type LineResource = (wgpu::Buffer, u32, u32);

#[derive(Default)]
pub(crate) struct Overlays {
    pub bindings: Option<OverlayBindings>,
    pub grid: Option<GridResource>,
    pub aabb: Vec<AabbResource>,
    pub axis: Vec<AxisResource>,
    pub probes: Option<LineResource>,
    pub balls: Vec<BallResource>,
    pub navmesh: Option<u32>,
    pub wires: Option<LineResource>,
}

impl Renderer {
    pub(crate) fn shared_bones_buffer(&self) -> &wgpu::Buffer {
        self.draw_buffers.default_bones()
    }

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

    pub(crate) fn precreate_game_overlays(&self, scene: &Scene, eye: Vec3) -> Overlays {
        let mut draws = OverlayDraws::default();
        let wires = self.precreate_wires(scene, (ColliderScope::All, eye), &mut draws);
        Overlays {
            bindings: self.overlay_bindings(&draws),
            wires,
            ..Default::default()
        }
    }

    pub(crate) fn precreate_solid_resources(
        &mut self,
        scene: &Scene,
        cam: &rusty_scene::Camera,
        (frustum, lod): (&Frustum, &LodSelection),
        (outline, history, order): (bool, Option<&mut MotionHistory>, OpaqueOrder),
    ) -> SolidResources {
        let eye = (cam.position, cam.forward());
        let (mut opaque, mut transparent, mut joints) = (Vec::new(), Vec::new(), Vec::new());
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
            draws: self.upload_solids((opaque, transparent, order), (&joints, &proxies.data)),
            culls,
            visible,
            mesh_particles,
            outline_bounds,
            tested,
            culled: None,
            floored: false,
        }
    }

    fn upload_solids(
        &mut self,
        (opaque, mut transparent, order): (
            Vec<DrawItem>,
            Vec<(DrawItem, TransparentOrder)>,
            OpaqueOrder,
        ),
        (joints, proxies): (&[JointMatrix], &[[f32; 4]]),
    ) -> FrameDraws {
        transparent_order(&mut transparent);
        let transparent = transparent.into_iter().map(|(item, _)| item).collect();
        let records = &self.retained.records;
        let draws = FrameDraws::build((opaque, transparent), records, (self.instancing, order));
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
