//! src/scene/asset_instance/collision.rs — the collision mesh a Mesh-shaped collider
//! builds from when it is not its entity's own (#1960): Unity's
//! `MeshCollider.sharedMesh` set to a low-poly copy, Unreal's `UCX_` mesh.
//!
//! A model's sidecar names it per sub-object (`asset::SubObjectCollider::mesh`), and
//! `Physics.SetColliderShape(id, { kind = "Mesh", mesh = .. })` names it in script.
//! Either way the collider keeps it as its `collision_mesh`, which physics, the
//! navmesh and the collider gizmo read ahead of the entity's Mesh
//! (`physics::shape::build::collider_inputs`); only its reference is saved, and the
//! geometry is rebuilt from it on load, re-import and undo (`scene::rehydrate`). A
//! name the model lacks is said once in the console and kept with no geometry: the
//! collider falls back to its entity's own mesh, as Unity's `MeshCollider` does with
//! a missing mesh reference, and picks the name up once the model has it.

use std::path::Path;

use crate::asset::{self, AssetRef, ImportedAsset};
use crate::components::{ColliderShape, SharedMesh};
use crate::scene::rehydrate::{bounds_of, refit_collider, warn_once_of};
use crate::scene::Scene;

/// The collision mesh `name` of the model at `path` (imported as `asset`): its
/// reference, with its geometry when the model has such a sub-object with triangles,
/// and without (warned once) when it has not.
pub fn collision_mesh(path: &str, asset: &ImportedAsset, name: &str) -> SharedMesh {
    let reference = format!("{path}{}{name}", asset::REF_SEPARATOR);
    let sub = asset.sub_mesh(name).filter(|s| !s.indices.is_empty());
    let mut mesh = reference_only(reference.clone());
    match sub.filter(|s| s.bounds().is_some()) {
        Some(sub) => {
            mesh.positions = sub.vertices.iter().map(|v| v.position).collect();
            mesh.indices = sub.indices.clone();
        }
        None => warn_unused(&reference, "the model has no such mesh"),
    }
    mesh
}

/// A collision mesh naming `reference`, its geometry not yet built.
fn reference_only(reference: String) -> SharedMesh {
    SharedMesh {
        primitive_type: "Asset".to_string(),
        asset_ref: Some(reference),
        ..SharedMesh::default()
    }
}

/// Say once per `reference` that it builds no collider, and why.
fn warn_unused(reference: &str, why: &str) {
    warn_once_of(reference, || {
        format!("collision mesh '{reference}' is not used ({why}): its collider builds from its own mesh")
    });
}

/// Make entity `id`'s collider a Mesh shape (`convex`: one hull), built from the
/// collision mesh `mesh` names, else from its own Mesh, else from the copy it kept
/// when its Mesh was removed. `mesh` is a `path::sub_object` reference, or a bare
/// sub-object of the model the entity's Mesh (or that kept copy) comes from; a name
/// the model lacks is kept and warned of once, and the collider falls back. The one
/// write `Physics.SetColliderShape` makes for a `"Mesh"` shape. Errors, changing
/// nothing, when the entity has no collider or nothing to build the shape from.
pub fn set_mesh_shape(
    scene: &mut Scene,
    id: u32,
    convex: bool,
    mesh: Option<&str>,
) -> Result<(), String> {
    let own = scene.world.mesh(id).map(|m| {
        let bounds = bounds_of(m.vertices.iter().map(|v| v.position));
        (bounds, m.asset_ref.clone())
    });
    let Some(mut collider) = scene.world.collider_mut(id) else {
        return Err(format!("entity {id} has no Collider"));
    };
    let (own_bounds, own_ref) = own.unwrap_or_default();
    let kept = collider.shared_mesh.as_ref();
    let model = own_ref.or_else(|| kept.and_then(|k| k.asset_ref.clone()));
    let named = mesh.and_then(|name| named_mesh(model.as_deref(), name));
    let builds = named.as_ref().is_some_and(|n| !n.indices.is_empty())
        || own_bounds.is_some()
        || kept.is_some_and(|k| !k.indices.is_empty());
    if !builds {
        return Err(format!(
            "entity {id}: a Mesh collider needs a Mesh or a collision `mesh` to build from"
        ));
    }
    collider.shape = ColliderShape::Mesh {
        convex,
        local_min: glam::Vec3::ZERO,
        local_max: glam::Vec3::ZERO,
    };
    collider.collision_mesh = named;
    refit_collider(&mut collider, own_bounds);
    Ok(())
}

/// The collision mesh `name` resolves to (a full reference, or a sub-object of the
/// file `model`, a `path::sub_object` reference, names), or `None` (warned once)
/// when it resolves to no file.
fn named_mesh(model: Option<&str>, name: &str) -> Option<SharedMesh> {
    let reference = if name.contains(asset::REF_SEPARATOR) {
        name.to_string()
    } else {
        let Some(model) = model.and_then(|r| AssetRef::parse(r).ok()) else {
            warn_unused(
                name,
                "the entity's mesh comes from no model to look it up in",
            );
            return None;
        };
        format!("{}{}{name}", model.path, asset::REF_SEPARATOR)
    };
    let parsed = match AssetRef::parse(&reference) {
        Ok(r) => r,
        Err(e) => {
            warn_unused(&reference, &e.to_string());
            return None;
        }
    };
    match asset::imported(Path::new(&parsed.path)) {
        Ok(asset) => Some(collision_mesh(&parsed.path, &asset, &parsed.sub_object)),
        Err(e) => {
            warn_unused(&reference, &e.to_string());
            Some(reference_only(reference))
        }
    }
}

#[cfg(test)]
#[path = "collision_fallback_tests.rs"]
mod fallback_tests;
#[cfg(test)]
#[path = "collision_tests.rs"]
pub(crate) mod tests;
