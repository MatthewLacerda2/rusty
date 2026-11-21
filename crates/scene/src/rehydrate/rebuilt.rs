use glam::Mat4;

use rusty_asset::mesh_data::Shared;
use rusty_asset::{MeshRevision, SubMesh};
use rusty_components::mesh::Vertex;

#[derive(Default)]
pub(super) struct RehydratedMesh {
    pub(super) vertices: Shared<Vertex>,
    pub(super) indices: Shared<u32>,
    pub(super) ranges: Vec<std::ops::Range<u32>>,
    pub(super) bind_palette: Vec<Mat4>,
    pub(super) skin: Option<rusty_asset::SkinData>,
    pub(super) clips: Vec<rusty_asset::AnimationClip>,
    pub(super) revision: Option<MeshRevision>,
    pub(super) clusters: Option<std::sync::Arc<rusty_asset::clusters::Clusters>>,
}

impl RehydratedMesh {
    pub(super) fn primitive(geometry: crate::create::Geometry, revision: MeshRevision) -> Self {
        Self {
            vertices: geometry.0,
            indices: geometry.1,
            revision: Some(revision),
            ..Self::default()
        }
    }

    pub(super) fn from_sub_mesh(sub: &SubMesh, revision: MeshRevision) -> Self {
        let bind_palette = sub
            .skin
            .as_ref()
            .map(|s| s.bind_palette())
            .unwrap_or_default();
        Self {
            vertices: sub.vertices.clone(),
            indices: sub.indices.clone(),
            ranges: sub.ranges.iter().map(|r| r.indices.clone()).collect(),
            bind_palette,
            skin: sub.skin.clone(),
            clips: sub.clips.clone(),
            revision: Some(revision),
            clusters: sub.clusters.clone(),
        }
    }
}
