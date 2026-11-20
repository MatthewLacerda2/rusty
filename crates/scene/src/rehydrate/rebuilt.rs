//! crates/scene/src/rehydrate/rebuilt.rs — the runtime data a mesh is rebuilt with.

use glam::Mat4;

use rusty_asset::{MeshRevision, MeshVertex, SubMesh};
use rusty_components::mesh::Vertex;

/// Convert one imported `MeshVertex` (pure data) into the renderer's `Vertex`,
/// carrying the skin binding (`joint_indices`/`joint_weights`) through to the GPU
/// vertex; static meshes keep the unskinned identity binding (#79).
fn vertex_from_imported(v: &MeshVertex) -> Vertex {
    Vertex {
        position: v.position,
        normal: v.normal,
        tex_coords: v.tex_coords,
        joint_indices: v.joint_indices,
        joint_weights: v.joint_weights,
        tangent: v.tangent,
        lightmap_uv: v.lightmap_uv,
    }
}

/// The non-serialized data a mesh component is rebuilt with on load: GPU-ready
/// geometry plus the imported rig (bind palette, skeleton, clips). A primitive or a
/// failed import yields the empty default — no skin, no clips, GPU bones at identity.
#[derive(Default)]
pub(super) struct RehydratedMesh {
    pub(super) vertices: Vec<Vertex>,
    pub(super) indices: Vec<u32>,
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
    pub(super) fn primitive(geometry: (Vec<Vertex>, Vec<u32>), revision: MeshRevision) -> Self {
        Self {
            vertices: geometry.0,
            indices: geometry.1,
            revision: Some(revision),
            ..Self::default()
        }
    }

    /// Pull geometry + the full rig (bind palette, skeleton, clips) out of an
    /// imported sub-mesh, which carries `revision`, its import's.
    pub(super) fn from_sub_mesh(sub: &SubMesh, revision: MeshRevision) -> Self {
        let bind_palette = sub
            .skin
            .as_ref()
            .map(|s| s.bind_palette())
            .unwrap_or_default();
        Self {
            vertices: sub.vertices.iter().map(vertex_from_imported).collect(),
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
