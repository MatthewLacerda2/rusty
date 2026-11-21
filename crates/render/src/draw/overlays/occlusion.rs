use glam::Vec3;
use wgpu::util::DeviceExt;

use super::{aabb_wireframe, OverlayDraws};
use crate::draw::resources::AabbResource;
use crate::Renderer;
use rusty_scene::Scene;

const CELL: [f32; 4] = [1.0, 0.85, 0.1, 1.0];
const HIDDEN: [f32; 4] = [1.0, 0.2, 0.15, 0.9];

impl Renderer {
    pub(crate) fn precreate_occlusion(
        &self,
        scene: &Scene,
        draws: &mut OverlayDraws,
    ) -> Vec<AabbResource> {
        let mut out = Vec::new();
        let Some((eye, mask)) = selected_camera(scene) else {
            return out;
        };
        let Some(rs) = self.retained.scene(scene.id()) else {
            return out;
        };
        let pvs = &rs.occlusion;
        let (Some(row), Some(cell)) = (pvs.row(eye, mask), pvs.cell_at(eye)) else {
            return out;
        };
        out.push(self.wire(cell, CELL, draws));
        for (id, object) in rs.objects() {
            if let Some(bounds) = object.bounds.filter(|_| pvs.hides(row, id)) {
                out.push(self.wire(bounds, HIDDEN, draws));
            }
        }
        out
    }

    fn wire(
        &self,
        (min, max): (Vec3, Vec3),
        tint: [f32; 4],
        draws: &mut OverlayDraws,
    ) -> AabbResource {
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Occlusion View"),
                contents: bytemuck::cast_slice(&aabb_wireframe(min, max)),
                usage: wgpu::BufferUsages::VERTEX,
            });
        (buffer, draws.push(glam::Mat4::IDENTITY, tint))
    }
}

fn selected_camera(scene: &Scene) -> Option<(Vec3, u32)> {
    let id = scene.selected_entity_id?;
    let cam = scene.world.camera(id)?;
    let eye = scene.world.local_to_world(id).w_axis.truncate();
    let culls = rusty_scene::lighting::occlusion::culls_from(cam.projection, cam.near);
    (cam.use_occlusion_culling && culls).then_some((eye, cam.culling_mask))
}
