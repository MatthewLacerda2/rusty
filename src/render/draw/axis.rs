//! Pre-creation of the selected entity's axis arrows. Extracted verbatim from
//! `Renderer::render` (behavior unchanged).

use glam::Mat4;
use wgpu::util::DeviceExt;

use crate::render::draw::resources::AxisResource;
use crate::render::{EntityUniform, Renderer};
use crate::scene::Scene;

impl Renderer {
    pub(crate) fn precreate_axis_arrows(&self, scene: &Scene) -> Vec<AxisResource> {
        let mut axis_arrow_resources = Vec::new();
        let Some(selected_id) = scene.selected_entity_id else {
            return axis_arrow_resources;
        };
        if !scene.world.contains(selected_id) {
            return axis_arrow_resources;
        }
        let world_matrix = scene.compute_world_matrix(selected_id);
        let world_pos = world_matrix.col(3).truncate();
        let arrow_model_matrix = Mat4::from_translation(world_pos);

        let colors = [
            [1.0, 0.1, 0.1, 1.0], // X: Red
            [0.1, 0.9, 0.1, 1.0], // Y: Green
            [0.1, 0.4, 1.0, 1.0], // Z: Blue
        ];

        for (i, color) in colors.iter().enumerate() {
            axis_arrow_resources.push(self.build_axis_arrow(i, arrow_model_matrix, *color));
        }
        axis_arrow_resources
    }

    /// Build one axis-arrow's uniform buffer + bind group (bound against the shared
    /// identity bone palette).
    fn build_axis_arrow(
        &self,
        i: usize,
        arrow_model_matrix: Mat4,
        color: [f32; 4],
    ) -> AxisResource {
        let entity_uniform = EntityUniform {
            model_matrix: arrow_model_matrix.to_cols_array(),
            color_tint: color,
            use_texture: 0,
            is_lit: 0,
            metallic: 0.0,
            roughness: 0.5,
            use_metallic_map: 0,
            use_roughness_map: 0,
            use_normal_map: 0,
            use_emissive_map: 0,
            emissive: [0.0; 4],
            use_sh: 0,
            use_cutout: 0,
            alpha_cutoff: 0.0,
            _sh_pad: 0,
            sh: [[0.0; 4]; 9],
        };

        let entity_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Axis Arrow Uniform"),
                contents: bytemuck::bytes_of(&entity_uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bind_group =
            self.entity_bind_group("Axis Arrow", &entity_buf, self.shared_bones_buffer());
        (i, entity_buf, bind_group)
    }
}
