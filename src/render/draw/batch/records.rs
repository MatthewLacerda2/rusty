//! One camera's batches as what the GPU reads (#1616): a per-draw uniform per batch,
//! in `uniform_slot` order, and the distinct material records they index.

use crate::render::gpu::uniforms::RecordTable;
use crate::render::{DrawUniform, MaterialData};

use super::FrameDraws;

impl FrameDraws {
    /// Every batch's per-draw uniform, in `uniform_slot` order, and the material
    /// records they name: each distinct record once, sorted by its words
    /// ([`RecordTable`]), so the array does not follow the draw order a moving camera
    /// changes.
    pub(crate) fn pack(&self) -> (Vec<DrawUniform>, Vec<MaterialData>) {
        let table = RecordTable::new(self.batches().map(|b| b.key.record));
        let uniforms = self
            .batches()
            .map(|b| DrawUniform {
                model_matrix: glam::Mat4::IDENTITY.to_cols_array(),
                bone_base: b.key.bone_base,
                material: table.index(&b.key.record),
                _pad: [0; 2],
            })
            .collect();
        (uniforms, table.records())
    }
}
