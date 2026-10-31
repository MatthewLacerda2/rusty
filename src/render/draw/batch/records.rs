//! One camera's batches as what the GPU reads (#1616): a per-draw uniform per batch,
//! in `uniform_slot` order, and the distinct material records they index.

use crate::core::collections::Map;
use crate::render::{DrawUniform, MaterialData};

use super::FrameDraws;

impl FrameDraws {
    /// Every batch's per-draw uniform, in `uniform_slot` order, and the material
    /// records they name. A record is keyed by its packed words and stored once, in
    /// first-draw order, so batches with identical material values share it and a
    /// scene that holds still packs the same array every frame.
    pub(crate) fn pack(&self) -> (Vec<DrawUniform>, Vec<MaterialData>) {
        let mut index = Map::default();
        let mut records = Vec::new();
        let uniforms = self
            .batches()
            .map(|b| {
                let material = *index.entry(b.key.record).or_insert_with(|| {
                    records.push(b.key.record());
                    records.len() as u32 - 1
                });
                DrawUniform {
                    model_matrix: glam::Mat4::IDENTITY.to_cols_array(),
                    bone_base: b.key.bone_base,
                    material,
                    _pad: [0; 2],
                }
            })
            .collect();
        (uniforms, records)
    }
}
