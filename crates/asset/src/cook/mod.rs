//! crates/asset/src/cook/ — what an import derives from a parsed model (#2332): the
//! steps after the parse, each run per sub-mesh.
//!
//! After the Scale Factor (`scale_factor`), each sub-mesh gets Generate Lightmap
//! UVs' unwrap (`lightmap_uv`, when its sidecar ticks it) and then its clusters
//! (`clusters`, last, since they reorder the indices every step before settled).
//! The sub-meshes run on threads ([`par`]), joined in document order: each step is
//! a pure function of its own sub-mesh and the settings, so the asset is the same
//! value, bit for bit, as one thread makes. The parse spreads its meshes the same
//! way (the tangents a glTF or `.obj` lacks are generated there).

pub mod par;

use super::mesh_data::ImportedAsset;
use super::{clusters, lightmap_uv, ImportSettings};

/// Run the per-sub-mesh steps on `asset`, already scaled, with `settings`.
pub(crate) fn derive(asset: &mut ImportedAsset, settings: &ImportSettings) {
    par::each_mut(&mut asset.sub_meshes, |sub| {
        if settings.lightmap_uvs.generate {
            lightmap_uv::apply(sub, &settings.lightmap_uvs);
        }
        clusters::apply(sub);
    });
}

#[cfg(test)]
mod fixture;
#[cfg(test)]
mod tests;
