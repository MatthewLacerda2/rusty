//! One camera's batches as what the GPU reads (#1616): a per-draw uniform per batch,
//! in `uniform_slot` order, each naming its material record's slot of the renderer's
//! material table (`render::scene::MaterialRecords`, #1706).

use crate::DrawUniform;

use super::FrameDraws;

impl FrameDraws {
    /// Every batch's per-draw uniform, in `uniform_slot` order. The records they name
    /// are kept slots, so nothing about them is packed per camera.
    pub(crate) fn pack(&self) -> Vec<DrawUniform> {
        self.batches()
            .map(|b| DrawUniform {
                model_matrix: glam::Mat4::IDENTITY.to_cols_array(),
                bone_base: b.key.bone_base,
                material: b.key.record,
                _pad: [0; 2],
            })
            .collect()
    }
}
