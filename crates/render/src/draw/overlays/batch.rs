use glam::Mat4;
use wgpu::util::DeviceExt;

use crate::gpu::draw_buffers::{draw_group, uniform_array_bytes, DrawBuffers};
use crate::gpu::groups;
use crate::gpu::uniforms::{RecordTable, RecordWords};
use crate::{DrawUniform, MaterialData, Renderer};

/// The overlays' gizmos in slot order: each one's placement and flat record.
#[derive(Default)]
pub(crate) struct OverlayDraws {
    gizmos: Vec<(Mat4, RecordWords)>,
}

impl OverlayDraws {
    pub(crate) fn push(&mut self, model_matrix: Mat4, tint: [f32; 4]) -> u32 {
        self.gizmos
            .push((model_matrix, MaterialData::flat(tint).words()));
        (self.gizmos.len() - 1) as u32
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.gizmos.len()
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

pub(crate) struct OverlayBindings {
    group: wgpu::BindGroup,
}

impl OverlayBindings {
    pub(crate) fn bind<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, slot: u32) {
        pass.set_bind_group(groups::DRAW, &self.group, &DrawBuffers::offsets(slot));
    }
}

impl Renderer {
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
        let group = draw_group(
            &self.device,
            &self.entity_bones_layout,
            "Overlay Bind Group",
            [
                &uniforms,
                self.shared_bones_buffer(),
                self.draw_buffers.identity_instance(),
                &records,
                self.draw_buffers.proxies(),
                self.draw_buffers.objects(),
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
