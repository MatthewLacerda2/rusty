//! crates/scene/src/asset_instance/collision/mod.rs — the collision mesh a Mesh-shaped collider
//! builds from when it is not its entity's own (#1960): Unity's
//! `MeshCollider.sharedMesh` set to a low-poly copy, Unreal's `UCX_` mesh.
//!
//! A model's sidecar names it per sub-object (`rusty_asset::SubObjectCollider::mesh`), and
//! `Physics.SetColliderShape(id, { kind = "Mesh", mesh = .. })` names it in script.
//! Either way the collider keeps it as its `collision_mesh`, which physics, the
//! navmesh and the collider gizmo read ahead of the entity's Mesh
//! (`physics::shape::build::collider_inputs`); only its reference is saved, and the
//! geometry is rebuilt from it on load, re-import and undo (`scene::rehydrate`). A
//! name the model lacks is said once in the console and kept with no geometry: the
//! collider falls back to its entity's own mesh, as Unity's `MeshCollider` does with
//! a missing mesh reference, and picks the name up once the model has it.

use std::path::Path;

use crate::rehydrate::{bounds_of, refit_collider, warn_once_of};
use crate::Scene;
use rusty_asset::{AssetRef, ImportedAsset};
use rusty_components::{ColliderShape, SharedMesh};

/// The collision mesh `name` of the model at `path` (imported as `asset`): its
/// reference, with its geometry when the model has such a sub-object with triangles,
/// and without (warned once) when it has not.
pub fn collision_mesh(path: &str, asset: &ImportedAsset, name: &str) -> SharedMesh {
    let reference = format!("{path}{}{name}", rusty_asset::REF_SEPARATOR);
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

/// The warn-once key of a collision mesh `reference` that builds no collider: its
/// own, so a "loads empty" said earlier of the same reference does not swallow it.
pub(crate) fn warn_key(reference: &str) -> String {
    format!("collision mesh {reference}")
}

/// Say once per `reference` that it builds no collider, and why.
fn warn_unused(reference: &str, why: &str) {
    warn_once_of(&warn_key(reference), || {
        format!("collision mesh '{reference}' is not used ({why}): its collider builds from its own mesh")
    });
}

/// Make entity `id`'s collider a Mesh shape (`convex`: one hull), built from the
/// collision mesh `mesh` names, else from its own Mesh, else from the copy it kept
/// when its Mesh was removed. `mesh` is a `path::sub_object` reference, or a bare
/// sub-object of the model the entity's Mesh (or that kept copy) comes from; a name
/// the model lacks (or one that resolves to no model) is kept and warned of once,
/// and the collider falls back. The one write `Physics.SetColliderShape` makes for a
/// `"Mesh"` shape. An entity without a collider is left alone, as for every other
/// shape; one with nothing to build the shape from is an error, changing nothing.
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
        return Ok(());
    };
    let (own_bounds, own_ref) = own.unwrap_or_default();
    let kept = collider.shared_mesh.as_ref();
    let model = own_ref.or_else(|| kept.and_then(|k| k.asset_ref.clone()));
    let named = mesh.map(|name| named_mesh(model.as_deref(), name));
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

/// The collision mesh `name` resolves to: a full reference, or a sub-object of the
/// file `model` (a `path::sub_object` reference) names. One that resolves to no file
/// or no sub-object is kept as written with no geometry, warned once, so the
/// collider falls back and the name is saved.
fn named_mesh(model: Option<&str>, name: &str) -> SharedMesh {
    let reference = if name.contains(rusty_asset::REF_SEPARATOR) {
        name.to_string()
    } else {
        let Some(model) = model.and_then(|r| AssetRef::parse(r).ok()) else {
            warn_unused(
                name,
                "the entity's mesh comes from no model to look it up in",
            );
            return reference_only(name.to_string());
        };
        format!("{}{}{name}", model.path, rusty_asset::REF_SEPARATOR)
    };
    let parsed = match AssetRef::parse(&reference) {
        Ok(r) => r,
        Err(e) => {
            warn_unused(&reference, &e.to_string());
            return reference_only(reference);
        }
    };
    match rusty_asset::imported(Path::new(&parsed.path)) {
        Ok(asset) => collision_mesh(&parsed.path, &asset, &parsed.sub_object),
        Err(e) => {
            warn_unused(&reference, &e.to_string());
            reference_only(reference)
        }
    }
}

#[cfg(test)]
mod edge_tests;
#[cfg(test)]
mod fallback_tests;
#[cfg(test)]
pub(crate) mod tests;
