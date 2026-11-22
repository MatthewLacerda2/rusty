mod adopt;
mod collider;
mod counts;
mod imports;
mod rebuilt;
mod warned;

pub use adopt::{adopt_or_rehydrate, Keep};
pub use collider::{refit_collider, Which};
#[doc(hidden)]
pub use counts::rebuilds;
pub use imports::Imports;
use rebuilt::RehydratedMesh;
use warned::warn_once;
pub use warned::warn_once_of;
#[doc(hidden)]
pub use warned::warned_of;

use glam::Vec3;

use crate::create::{primitive_revision, subdivided_geometry, Primitive};
use rusty_asset::{MeshRevision, SubMesh};
use rusty_components::Entity;

#[derive(Clone)]
pub struct RehydratedScene {
    data: crate::serialize::SceneData,
    read_at: u64,
    imports: Imports,
}

impl RehydratedScene {
    pub fn new(data: crate::serialize::SceneData) -> Self {
        let mut scene = Self::read_since(data, 0);
        scene.read_at = rusty_core::fs::written::written_since(u64::MAX).0;
        scene
    }

    pub fn read_since(mut data: crate::serialize::SceneData, read_at: u64) -> Self {
        let mut imports = Imports::default();
        for entity in &mut data.entities {
            rehydrate_in(entity, &mut imports);
        }
        Self {
            data,
            read_at,
            imports,
        }
    }

    pub fn data_mut(&mut self) -> &mut crate::serialize::SceneData {
        &mut self.data
    }

    pub fn data(&self) -> &crate::serialize::SceneData {
        &self.data
    }

    pub fn into_parts(self) -> (crate::serialize::SceneData, u64, Imports) {
        (self.data, self.read_at, self.imports)
    }
}

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

fn rehydrate_one(mesh: &crate::MeshComponent, imports: &mut Imports) -> RehydratedMesh {
    if mesh.primitive_type == "Asset" {
        return rehydrate_asset_mesh(&mesh.asset_ref, imports);
    }
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

pub fn rehydrate_in(entity: &mut Entity, imports: &mut Imports) {
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

pub fn bounds_of(positions: impl Iterator<Item = [f32; 3]>) -> Option<(Vec3, Vec3)> {
    positions.map(Vec3::from_array).fold(None, |b, p| {
        Some(b.map_or((p, p), |(lo, hi): (Vec3, Vec3)| (lo.min(p), hi.max(p))))
    })
}

pub fn install_sub_mesh(mesh: &mut crate::MeshComponent, sub: &SubMesh, revision: MeshRevision) {
    install(mesh, RehydratedMesh::from_sub_mesh(sub, revision));
}

fn install(mesh: &mut crate::MeshComponent, rebuilt: RehydratedMesh) {
    counts::count_rebuild();
    mesh.vertices = rebuilt.vertices;
    mesh.indices = rebuilt.indices;
    mesh.ranges = rebuilt.ranges;
    mesh.bind_palette = rebuilt.bind_palette;
    mesh.skin = rebuilt.skin;
    mesh.clips = rebuilt.clips;
    mesh.pose_palette = Vec::new();
    mesh.skeleton.bones.clear();
    mesh.revision = rebuilt.revision.unwrap_or_else(MeshRevision::next);
    mesh.own_geometry = None;
    mesh.clusters = rebuilt.clusters.map(|c| (mesh.revision, c));
}

pub fn asset_mesh_component(reference: &str) -> crate::MeshComponent {
    built(
        reference,
        rehydrate_asset_mesh(&Some(reference.to_string()), &mut Imports::default()),
    )
}

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

pub fn asset_ref_file(reference: &str) -> Option<String> {
    rusty_asset::AssetRef::parse(reference).ok().map(|r| r.path)
}
