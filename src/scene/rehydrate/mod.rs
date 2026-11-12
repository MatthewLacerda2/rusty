//! src/scene/rehydrate/mod.rs — rebuild a mesh's runtime data from its on-disk reference.
//!
//! A saved mesh is a REFERENCE (a primitive name, or an asset's `path::sub_object`);
//! its geometry, bind palette, skeleton and clips are rebuilt here on load, prefab
//! stamp and propagation. Split out of `serialize` (#453) to keep it under the size
//! cap. GPU buffers are never stored on disk, only rebuilt here.

mod collider;
pub mod model_writes;
mod reimport;

pub(crate) use collider::{rebuild as rebuild_collider_mesh, refit_collider, Which};

pub use reimport::{
    reimport_model, reimport_model_except, reimport_model_ids, reimport_written, Rig,
};

use glam::{Mat4, Vec3};

use crate::asset::{self, MeshRevision, MeshVertex, SubMesh};
use crate::components::mesh::Vertex;
use crate::components::Entity;
use crate::scene::authoring::{subdivided_geometry, Primitive};

/// A document with every mesh rebuilt from its reference, and the write-log stamp
/// taken once they were (`meshes_read_at`, #1242): the file-only half of a load,
/// which never touches a live scene (#2123).
pub struct RehydratedScene {
    data: crate::scene::serialize::SceneData,
    read_at: u64,
}

impl RehydratedScene {
    pub fn new(mut data: crate::scene::serialize::SceneData) -> Self {
        data.entities.iter_mut().for_each(rehydrate_entity_mesh);
        let read_at = crate::core::fs::written::written_since(u64::MAX).0;
        Self { data, read_at }
    }

    /// The document, for the id moves a load makes before applying it
    /// (`scene::runtime::load::make_room`): ids are not what rehydration reads.
    pub(crate) fn data_mut(&mut self) -> &mut crate::scene::serialize::SceneData {
        &mut self.data
    }

    /// The document and its stamp, for `serialize`'s apply.
    pub(crate) fn into_parts(self) -> (crate::scene::serialize::SceneData, u64) {
        (self.data, self.read_at)
    }
}

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
struct RehydratedMesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    ranges: Vec<std::ops::Range<u32>>,
    bind_palette: Vec<Mat4>,
    skin: Option<crate::asset::SkinData>,
    clips: Vec<crate::asset::AnimationClip>,
    /// The revision of the import it came from (#857): every instance of one
    /// import shares it. `None` (a primitive, a failed import) takes a fresh one.
    revision: Option<MeshRevision>,
    /// The import's clusters (#1968), which describe `revision`'s geometry.
    clusters: Option<std::sync::Arc<crate::asset::clusters::Clusters>>,
}

impl RehydratedMesh {
    /// A primitive's geometry with no rig — primitives are never skinned/animated.
    fn primitive(geometry: (Vec<Vertex>, Vec<u32>)) -> Self {
        Self {
            vertices: geometry.0,
            indices: geometry.1,
            ..Self::default()
        }
    }

    /// Pull geometry + the full rig (bind palette, skeleton, clips) out of an
    /// imported sub-mesh, which carries `revision`, its import's.
    fn from_sub_mesh(sub: &SubMesh, revision: MeshRevision) -> Self {
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

/// Re-import an `"Asset"` mesh's geometry + rig from its path-based `asset_ref`
/// (`path::sub_object`), through the file's shared import (`asset::imported`, #857),
/// so a load instancing one file many times parses it once. On failure
/// (missing/renamed source — the accepted path-based trade-off) the mesh
/// rehydrates empty rather than aborting the load.
fn rehydrate_asset_mesh(asset_ref: &Option<String>) -> RehydratedMesh {
    match asset_ref {
        Some(reference) => match shared_sub_mesh(reference) {
            Ok(rebuilt) => rebuilt,
            Err(e) => {
                warn_once(reference, &e);
                RehydratedMesh::default()
            }
        },
        None => RehydratedMesh::default(),
    }
}

/// The sub-mesh `reference` names, out of its file's shared import.
fn shared_sub_mesh(reference: &str) -> Result<RehydratedMesh, asset::ImportError> {
    let asset_ref = asset::AssetRef::parse(reference)?;
    let import = asset::imported(std::path::Path::new(&asset_ref.path))?;
    let sub = import.sub_mesh(&asset_ref.sub_object).ok_or_else(|| {
        asset::ImportError::BadReference(format!("'{reference}': the file has no such sub-object"))
    })?;
    Ok(RehydratedMesh::from_sub_mesh(sub, import.revision))
}

thread_local! {
    /// The references a failed rehydrate was already warned of: a scene reloads
    /// often, and one warning per reference is enough (#1295).
    static WARNED: std::cell::RefCell<std::collections::BTreeSet<String>> =
        const { std::cell::RefCell::new(std::collections::BTreeSet::new()) };
}

/// Say why `reference`'s mesh comes back empty, once per reference, rather than
/// leaving an invisible entity with no explanation.
fn warn_once(reference: &str, error: &asset::ImportError) {
    warn_once_of(reference, || {
        format!("mesh '{reference}' loads empty: {error}")
    });
}

/// Say `message` once per `key` in the console, out of the set every failed
/// rehydrate shares: a scene reloads often, and once is enough.
pub(crate) fn warn_once_of(key: &str, message: impl FnOnce() -> String) {
    if WARNED.with(|warned| warned.borrow_mut().insert(key.to_string())) {
        log::warn!("[Scene] {}", message());
    }
}

/// Whether `key` was already warned of ([`warn_once_of`]).
#[cfg(test)]
pub(crate) fn warned_of(key: &str) -> bool {
    WARNED.with(|warned| warned.borrow().contains(key))
}

/// Rebuild one mesh's vertex/index data + rig from its on-disk REFERENCE: a
/// primitive from `primitive_type`, or an imported sub-mesh from `asset_ref` when
/// the type is `"Asset"`. GPU buffers are never stored on disk, only rebuilt here.
fn rehydrate_one(mesh: &crate::scene::MeshComponent) -> RehydratedMesh {
    if mesh.primitive_type == "Asset" {
        return rehydrate_asset_mesh(&mesh.asset_ref);
    }
    // Primitive geometry is owned by `scene::authoring` so a created primitive and
    // a loaded one are byte-identical.
    let primitive = Primitive::parse(&mesh.primitive_type);
    match primitive.and_then(|p| subdivided_geometry(p, mesh.subdivisions)) {
        Some(geometry) => RehydratedMesh::primitive(geometry),
        None => RehydratedMesh::default(),
    }
}

/// Rebuild one entity's mesh (if any) from its on-disk reference. Shared by scene
/// load and prefab instantiate (#215) so both rehydrate identically; GPU buffers
/// are never stored on disk, only rebuilt here.
/// A Mesh-shaped collider's shared mesh (#1657, when the entity has no Mesh) and
/// collision mesh (#1960) are rebuilt the same way, and its saved box is refit to
/// what it builds from (`collider::refit_collider`), so an import setting changed
/// since the save (the Scale Factor, #1398) reaches it.
pub fn rehydrate_entity_mesh(entity: &mut Entity) {
    if let Some(mesh) = &mut entity.mesh {
        hold_fields(entity.id, mesh);
        let rebuilt = rehydrate_one(mesh);
        install(mesh, rebuilt);
    }
    let Some(collider) = &mut entity.collider else {
        return;
    };
    if entity.mesh.is_none() {
        collider::rebuild(entity.id, collider, collider::Which::Kept);
    }
    collider::rebuild(entity.id, collider, collider::Which::Collision);
    let own = entity.mesh.as_ref();
    refit_collider(
        collider,
        own.and_then(|m| bounds_of(m.vertices.iter().map(|v| v.position))),
    );
}

/// Hold the Mesh fields the field table leaves `unheld!`: clamp `mesh`'s
/// subdivisions to what its primitive takes and its Scale In Lightmap to its range
/// (#1704), warning for each it had to move.
fn hold_fields(id: u32, mesh: &mut crate::scene::MeshComponent) {
    use crate::scene::authoring::rendering::mesh;
    let warnings = [
        mesh::hold_subdivisions(mesh),
        mesh::hold_scale_in_lightmap(mesh),
    ];
    for warning in warnings.into_iter().flatten() {
        log::warn!("[Scene] entity {id}: {warning}");
    }
}

/// The box around `positions`, or `None` for none.
pub(crate) fn bounds_of(positions: impl Iterator<Item = [f32; 3]>) -> Option<(Vec3, Vec3)> {
    positions.map(Vec3::from_array).fold(None, |b, p| {
        Some(b.map_or((p, p), |(lo, hi): (Vec3, Vec3)| (lo.min(p), hi.max(p))))
    })
}

/// Put `rebuilt`'s geometry and rig on `mesh`, unbound from any bones: a fresh
/// mesh starts at its rest pose, and the animation system repopulates the posed
/// palette once a clip plays. `Scene::sync_skeleton` binds the bones.
fn install(mesh: &mut crate::scene::MeshComponent, rebuilt: RehydratedMesh) {
    mesh.vertices = rebuilt.vertices;
    mesh.indices = rebuilt.indices;
    mesh.ranges = rebuilt.ranges;
    mesh.bind_palette = rebuilt.bind_palette;
    mesh.skin = rebuilt.skin;
    mesh.clips = rebuilt.clips;
    mesh.pose_palette = Vec::new();
    mesh.skeleton.bones.clear();
    mesh.revision = rebuilt.revision.unwrap_or_else(MeshRevision::next);
    mesh.clusters = rebuilt.clusters.map(|c| (mesh.revision, c));
}

/// Build an `"Asset"` mesh component from a path-based reference, importing its
/// geometry up front. The reference is the only identity stored; on save the
/// vertices are dropped and re-imported from it (see `RehydratedScene::new`). Returns
/// an empty-geometry component if the import fails.
pub fn asset_mesh_component(reference: &str) -> crate::scene::MeshComponent {
    built(
        reference,
        rehydrate_asset_mesh(&Some(reference.to_string())),
    )
}

/// [`asset_mesh_component`] from `sub`, the sub-mesh `reference` names, which
/// its caller already imported (as an import carrying `revision`): no second parse
/// of the file.
pub fn asset_mesh_component_of(
    reference: &str,
    sub: &SubMesh,
    revision: MeshRevision,
) -> crate::scene::MeshComponent {
    built(reference, RehydratedMesh::from_sub_mesh(sub, revision))
}

fn built(reference: &str, rebuilt: RehydratedMesh) -> crate::scene::MeshComponent {
    let mut mesh = crate::scene::MeshComponent {
        asset_ref: Some(reference.to_string()),
        ..crate::scene::MeshComponent::new("Asset", Vec::new(), Vec::new())
    };
    install(&mut mesh, rebuilt);
    mesh
}

/// The file a mesh reference rehydrates from (`models/a.fbx::Body` →
/// `models/a.fbx`): what a renderer cache keyed by reference matches against the
/// write log (#1219). `None` for a string that is not a reference (a primitive).
pub fn asset_ref_file(reference: &str) -> Option<String> {
    asset::AssetRef::parse(reference).ok().map(|r| r.path)
}

#[cfg(test)]
mod shared_tests;
#[cfg(test)]
mod tests;
