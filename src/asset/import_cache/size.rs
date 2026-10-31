//! src/asset/import_cache/size.rs — what an import's geometry costs to hold, the
//! measure of the cache's budget (#857).

use crate::asset::mesh_data::ImportedAsset;

/// About how much memory an import's geometry and animation keys take.
pub fn approx_bytes(asset: &ImportedAsset) -> usize {
    use std::mem::size_of;
    let keys = |c: &crate::asset::AnimationClip| -> usize {
        let t = c.tracks.iter();
        t.map(|t| {
            let times = t.translation.times.len() + t.rotation.times.len() + t.scale.times.len();
            times * size_of::<f32>()
                + (t.translation.values.len() + t.scale.values.len()) * size_of::<glam::Vec3>()
                + t.rotation.values.len() * size_of::<glam::Quat>()
        })
        .sum()
    };
    let subs = asset.sub_meshes.iter().map(|s| {
        s.vertices.len() * size_of::<crate::asset::MeshVertex>()
            + s.indices.len() * size_of::<u32>()
            + s.clips.iter().map(keys).sum::<usize>()
    });
    subs.sum()
}
