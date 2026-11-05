//! The editor's occlusion view (#1714), Unity's Occlusion Culling *Visualization*:
//! with a camera selected, the Scene view outlines the baked PVS cell that camera
//! stands in (yellow) and every static mesh the PVS hides from it (red), over
//! whatever the editor's own camera shows. Nothing is drawn with no usable bake, a
//! camera with `use_occlusion_culling` off, one that culls nothing by occlusion
//! (orthographic, or a far near plane), or one outside every cell.

use glam::Vec3;
use wgpu::util::DeviceExt;

use super::{aabb_wireframe, OverlayDraws};
use crate::render::draw::resources::AabbResource;
use crate::render::Renderer;
use crate::scene::Scene;

/// The cell's outline and the hidden meshes' tints.
const CELL: [f32; 4] = [1.0, 0.85, 0.1, 1.0];
const HIDDEN: [f32; 4] = [1.0, 0.2, 0.15, 0.9];

impl Renderer {
    /// The selected camera's cell and what its PVS row hides, as AABB wireframes.
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

/// The selected entity's camera, when it culls by occlusion as the renderer's
/// `occlusion_eye` decides: where it stands and the layers it draws.
fn selected_camera(scene: &Scene) -> Option<(Vec3, u32)> {
    let id = scene.selected_entity_id?;
    let cam = scene.world.camera(id)?;
    let eye = scene.world.local_to_world(id).w_axis.truncate();
    let culls = crate::scene::lighting::occlusion::culls_from(cam.projection, cam.near);
    (cam.use_occlusion_culling && culls).then_some((eye, cam.culling_mask))
}
