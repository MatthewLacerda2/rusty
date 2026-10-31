//! Pre-creation of the selected entity's axis arrows. Extracted verbatim from
//! `Renderer::render` (behavior unchanged).

use glam::Mat4;

use crate::render::draw::resources::AxisResource;
use crate::render::Renderer;
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
        let world_pos = scene.world.world_position(selected_id);
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
        let (entity_buf, bind_group) =
            self.overlay_bindings("Axis Arrow", arrow_model_matrix, color);
        (i, entity_buf, bind_group)
    }
}
