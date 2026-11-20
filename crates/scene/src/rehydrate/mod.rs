//! crates/scene/src/rehydrate/mod.rs — rebuild a mesh's runtime data from its on-disk reference.
//!
//! A saved mesh is a REFERENCE (a primitive name, or an asset's `path::sub_object`);
//! its geometry, bind palette, skeleton and clips are rebuilt here on load, prefab
//! stamp and propagation. Split out of `serialize` (#453) to keep it under the size
//! cap. GPU buffers are never stored on disk, only rebuilt here.
//!
//! Rebuilding what a written model changed (`reimport`, `model_writes`) is the edit
//! side's (`rusty_edit::rehydrate`): it re-baselines prefab instances.

mod adopt;
mod collider;
mod imports;
mod rebuilt;
mod warned;

pub(crate) use adopt::take_live;
pub use collider::{refit_collider, Which};
pub(crate) use imports::Imports;
use rebuilt::RehydratedMesh;
use warned::warn_once;
pub use warned::warn_once_of;
#[doc(hidden)]
pub use warned::warned_of;

use glam::Vec3;

use crate::create::{primitive_revision, subdivided_geometry, Primitive};
use rusty_asset::{MeshRevision, SubMesh};
use rusty_components::Entity;

/// A document with every mesh rebuilt from its reference, and the write-log stamp
/// taken once they were (`meshes_read_at`, #1242): the file-only half of a load,
/// which never touches a live scene (#2123).
#[derive(Clone)]
pub struct RehydratedScene {
    data: crate::serialize::SceneData,
    read_at: u64,
}

impl RehydratedScene {
    pub fn new(data: crate::serialize::SceneData) -> Self {
        let mut scene = Self::read_since(data, 0);
        scene.read_at = rusty_core::fs::written::written_since(u64::MAX).0;
        scene
    }

    /// [`Self::new`] stamped `read_at`, taken before the document was read: a load
    /// off the ticking thread (#2123) still follows a model written while it ran.
    /// Each model file is looked up once for the whole document ([`Imports`]).
    pub fn read_since(mut data: crate::serialize::SceneData, read_at: u64) -> Self {
        let mut imports = Imports::default();
        for entity in &mut data.entities {
            rehydrate_in(entity, &mut imports);
        }
        Self { data, read_at }
    }

    /// The document, for the id moves a load makes before applying it
    /// (`scene::runtime::load::make_room`): ids are not what rehydration reads.
    pub fn data_mut(&mut self) -> &mut crate::serialize::SceneData {
        &mut self.data
    }

    /// The document, rehydrated: what a load's GPU warm-up reads (`render::warm`).
    pub fn data(&self) -> &crate::serialize::SceneData {
        &self.data
    }

    /// The document and its stamp, for `serialize`'s apply.
    pub fn into_parts(self) -> (crate::serialize::SceneData, u64) {
        (self.data, self.read_at)
    }
}

/// Re-import an `"Asset"` mesh's geometry + rig from its path-based `asset_ref`
/// (`path::sub_object`), through the file's shared import (`rusty_asset::imported`, #857),
/// so a load instancing one file many times parses it once. On failure
/// (missing/renamed source — the accepted path-based trade-off) the mesh
/// rehydrates empty rather than aborting the load.
fn rehydrate_asset_mesh(asset_ref: &Option<String>, imports: &mut Imports) -> RehydratedMesh {
    match asset_ref {
        Some(reference) => match shared_sub_mesh(reference, imports) {
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
fn shared_sub_mesh(
    reference: &str,
    imports: &mut Imports,
) -> Result<RehydratedMesh, rusty_asset::ImportError> {
    let asset_ref = rusty_asset::AssetRef::parse(reference)?;
    let import = imports.of(&asset_ref.path)?;
    let sub = import.sub_mesh(&asset_ref.sub_object).ok_or_else(|| {
        rusty_asset::ImportError::BadReference(format!(
            "'{reference}': the file has no such sub-object"
        ))
    })?;
    Ok(RehydratedMesh::from_sub_mesh(sub, import.revision))
}

/// Rebuild one mesh's vertex/index data + rig from its on-disk REFERENCE: a
/// primitive from `primitive_type`, or an imported sub-mesh from `asset_ref` when
/// the type is `"Asset"`. GPU buffers are never stored on disk, only rebuilt here.
fn rehydrate_one(mesh: &crate::MeshComponent, imports: &mut Imports) -> RehydratedMesh {
    if mesh.primitive_type == "Asset" {
        return rehydrate_asset_mesh(&mesh.asset_ref, imports);
    }
    // Primitive geometry is owned by `scene::create` so a created primitive and
    // a loaded one are byte-identical.
    let primitive = Primitive::parse(&mesh.primitive_type);
    let Some(primitive) = primitive else {
        return RehydratedMesh::default();
    };
    match subdivided_geometry(primitive, mesh.subdivisions) {
        Some(geometry) => {
            let revision = primitive_revision(primitive, mesh.subdivisions);
            RehydratedMesh::primitive(geometry, revision)
        }
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
    rehydrate_in(entity, &mut Imports::default());
}

/// [`rehydrate_entity_mesh`] within a pass that looks each file up once.
pub(crate) fn rehydrate_in(entity: &mut Entity, imports: &mut Imports) {
    if let Some(mesh) = &mut entity.mesh {
        hold_fields(entity.id, mesh);
        let rebuilt = rehydrate_one(mesh, imports);
        install(mesh, rebuilt);
    }
    let Some(collider) = &mut entity.collider else {
        return;
    };
    if entity.mesh.is_none() {
        collider::rebuild(entity.id, collider, collider::Which::Kept, imports);
    }
    collider::rebuild(entity.id, collider, collider::Which::Collision, imports);
    let own = entity.mesh.as_ref();
    refit_collider(
        collider,
        own.and_then(|m| bounds_of(m.vertices.iter().map(|v| v.position))),
    );
}

/// Hold the Mesh fields the field table leaves `unheld!`: clamp `mesh`'s
/// subdivisions to what its primitive takes and its Scale In Lightmap to its range
/// (#1704), warning for each it had to move.
fn hold_fields(id: u32, mesh: &mut crate::MeshComponent) {
    use crate::rules::mesh as rules;
    let warnings = [
        rules::hold_subdivisions(mesh),
        rules::hold_scale_in_lightmap(mesh),
    ];
    for warning in warnings.into_iter().flatten() {
        log::warn!("[Scene] entity {id}: {warning}");
    }
}

/// The box around `positions`, or `None` for none.
pub fn bounds_of(positions: impl Iterator<Item = [f32; 3]>) -> Option<(Vec3, Vec3)> {
    positions.map(Vec3::from_array).fold(None, |b, p| {
        Some(b.map_or((p, p), |(lo, hi): (Vec3, Vec3)| (lo.min(p), hi.max(p))))
    })
}

/// Put `sub`'s geometry and rig, imported at `revision`, on `mesh`: what a reimport
/// rebuilds a mesh drawing a written model with (`rusty_edit::rehydrate`).
pub fn install_sub_mesh(mesh: &mut crate::MeshComponent, sub: &SubMesh, revision: MeshRevision) {
    install(mesh, RehydratedMesh::from_sub_mesh(sub, revision));
}

/// Put `rebuilt`'s geometry and rig on `mesh`, unbound from any bones: a fresh
/// mesh starts at its rest pose, and the animation system repopulates the posed
/// palette once a clip plays. `Scene::sync_skeleton` binds the bones.
fn install(mesh: &mut crate::MeshComponent, rebuilt: RehydratedMesh) {
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
pub fn asset_mesh_component(reference: &str) -> crate::MeshComponent {
    built(
        reference,
        rehydrate_asset_mesh(&Some(reference.to_string()), &mut Imports::default()),
    )
}

/// [`asset_mesh_component`] from `sub`, the sub-mesh `reference` names, which
/// its caller already imported (as an import carrying `revision`): no second parse
/// of the file.
pub fn asset_mesh_component_of(
    reference: &str,
    sub: &SubMesh,
    revision: MeshRevision,
) -> crate::MeshComponent {
    built(reference, RehydratedMesh::from_sub_mesh(sub, revision))
}

fn built(reference: &str, rebuilt: RehydratedMesh) -> crate::MeshComponent {
    let mut mesh = crate::MeshComponent {
        asset_ref: Some(reference.to_string()),
        ..crate::MeshComponent::new("Asset", Vec::new(), Vec::new())
    };
    install(&mut mesh, rebuilt);
    mesh
}

/// The file a mesh reference rehydrates from (`models/a.fbx::Body` →
/// `models/a.fbx`): what a renderer cache keyed by reference matches against the
/// write log (#1219). `None` for a string that is not a reference (a primitive).
pub fn asset_ref_file(reference: &str) -> Option<String> {
    rusty_asset::AssetRef::parse(reference).ok().map(|r| r.path)
}
