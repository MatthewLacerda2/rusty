//! crates/scene/src/rehydrate/rebuilt.rs — the runtime data a mesh is rebuilt with.

use glam::Mat4;

use rusty_asset::mesh_data::Shared;
use rusty_asset::{MeshRevision, SubMesh};
use rusty_components::mesh::Vertex;

/// The non-serialized data a mesh component is rebuilt with on load: GPU-ready
/// geometry, shared with the import it came from (#2351), plus the imported rig (bind palette, skeleton, clips). A primitive or a
/// failed import yields the empty default — no skin, no clips, GPU bones at identity.
#[derive(Default)]
pub(super) struct RehydratedMesh {
    pub(super) vertices: Shared<Vertex>,
    pub(super) indices: Shared<u32>,
    pub(super) ranges: Vec<std::ops::Range<u32>>,
    pub(super) bind_palette: Vec<Mat4>,
    pub(super) skin: Option<rusty_asset::SkinData>,
    pub(super) clips: Vec<rusty_asset::AnimationClip>,
    /// The revision of the import (#857) or primitive it came from: every instance
    /// of one shares it. `None` (a failed import) takes a fresh one.
    pub(super) revision: Option<MeshRevision>,
    /// The import's clusters (#1968), which describe `revision`'s geometry.
    pub(super) clusters: Option<std::sync::Arc<rusty_asset::clusters::Clusters>>,
}

impl RehydratedMesh {
    /// A primitive's geometry with no rig — primitives are never skinned/animated —
    /// at its one revision (`create::primitive_revision`).
    pub(super) fn primitive(geometry: crate::create::Geometry, revision: MeshRevision) -> Self {
        Self {
            vertices: geometry.0,
            indices: geometry.1,
            revision: Some(revision),
            ..Self::default()
        }
    }

    /// Pull geometry + the full rig (bind palette, skeleton, clips) out of an
    /// imported sub-mesh, which carries `revision`, its import's. The vertex and
    /// index buffers are the sub-mesh's own, shared, never copied.
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
