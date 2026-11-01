//! The editor overlays' draws for one handles pass (#1616): every gizmo's per-draw
//! uniform in one array and every distinct tint's record in one material array,
//! behind one group-1 bind group, as the solids' `DrawBuffers` are. A gizmo keeps only
//! its uniform's slot and binds the group at that slot's dynamic offset, so a frame
//! with a hundred collider boxes builds three buffers, not two per box.

use glam::Mat4;
use wgpu::util::DeviceExt;

use crate::render::gpu::draw_buffers::{group1, uniform_array_bytes, DrawBuffers};
use crate::render::gpu::uniforms::{RecordTable, RecordWords};
use crate::render::{DrawUniform, MaterialData, Renderer};

/// The overlays' gizmos in slot order: each one's placement and flat record.
#[derive(Default)]
pub(crate) struct OverlayDraws {
    gizmos: Vec<(Mat4, RecordWords)>,
}

impl OverlayDraws {
    /// Add a gizmo placed by `model_matrix` and drawn flat in `tint`
    /// ([`MaterialData::flat`]); returns its slot. Gizmos of one tint share a record.
    pub(crate) fn push(&mut self, model_matrix: Mat4, tint: [f32; 4]) -> u32 {
        self.gizmos
            .push((model_matrix, MaterialData::flat(tint).words()));
        (self.gizmos.len() - 1) as u32
    }

    /// Every gizmo's per-draw uniform, in slot order, and the records they index
    /// ([`RecordTable`]: one per distinct tint).
    fn pack(&self) -> (Vec<DrawUniform>, Vec<MaterialData>) {
        let table = RecordTable::new(self.gizmos.iter().map(|g| g.1));
        let uniforms = self
            .gizmos
            .iter()
            .map(|(model, words)| DrawUniform {
                model_matrix: model.to_cols_array(),
                bone_base: 0,
                material: table.index(words),
                _pad: [0; 2],
            })
            .collect();
        (uniforms, table.records())
    }
}

/// The handles pass's group 1: the overlays' uniform and record arrays over the
/// shared identity bones and identity instance (an overlay is one draw of instance
/// 0, its transform in its uniform).
pub(crate) struct OverlayBindings {
    group: wgpu::BindGroup,
}

impl OverlayBindings {
    /// Bind the group for the gizmo at `slot`.
    pub(crate) fn bind<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, slot: u32) {
        pass.set_bind_group(1, &self.group, &DrawBuffers::offsets(slot));
    }
}

impl Renderer {
    /// Upload `draws` as one bind group, or `None` when no gizmo was added.
    pub(crate) fn overlay_bindings(&self, draws: &OverlayDraws) -> Option<OverlayBindings> {
        if draws.gizmos.is_empty() {
            return None;
        }
        let (uniforms, records) = draws.pack();
        let init = |label, contents: &[u8], usage| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents,
                    usage,
                })
        };
        let uniforms = init(
            "Overlay Draw Uniforms",
            &uniform_array_bytes(&uniforms),
            wgpu::BufferUsages::UNIFORM,
        );
        let records = init(
            "Overlay Materials",
            bytemuck::cast_slice(&records),
            wgpu::BufferUsages::STORAGE,
        );
        let group = group1(
            &self.device,
            &self.entity_bones_layout,
            "Overlay Bind Group",
            [
                &uniforms,
                self.shared_bones_buffer(),
                self.draw_buffers.identity_instance(),
                &records,
                self.draw_buffers.proxies(),
            ],
        );
        Some(OverlayBindings { group })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gizmos_of_one_tint_share_a_record_and_keep_their_own_slot() {
        let mut draws = OverlayDraws::default();
        let cyan = [0.0, 0.8, 1.0, 0.8];
        let a = draws.push(Mat4::IDENTITY, cyan);
        let b = draws.push(Mat4::from_translation(glam::Vec3::X), [1.0, 0.0, 0.0, 1.0]);
        let c = draws.push(Mat4::IDENTITY, cyan);
        assert_eq!((a, b, c), (0, 1, 2), "every gizmo has its own uniform");
        let (uniforms, records) = draws.pack();
        assert_eq!(records.len(), 2, "two tints, two records");
        assert_eq!(uniforms[2].material, uniforms[0].material);
        assert_ne!(uniforms[1].material, uniforms[0].material);
        assert_eq!(
            uniforms[1].model_matrix[12], 1.0,
            "placement rides the uniform"
        );
    }
}
