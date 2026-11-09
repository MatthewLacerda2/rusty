//! src/scene/asset_instance/collision.rs — the collision mesh a Mesh-shaped collider
//! builds from when it is not its entity's own (#1960): Unity's
//! `MeshCollider.sharedMesh` set to a low-poly copy, Unreal's `UCX_` mesh.
//!
//! A model's sidecar names it per sub-object (`asset::SubObjectCollider::mesh`), and
//! `Physics.SetColliderShape(id, { kind = "Mesh", mesh = .. })` names it in script.
//! Either way the collider keeps it as its `shared_mesh`, which physics, the navmesh
//! and the collider gizmo read ahead of the entity's Mesh
//! (`physics::shape::build::collider_inputs`); only its reference is saved, and the
//! geometry is rebuilt from it on load (`scene::rehydrate`). A name the model lacks
//! is said once in the console and the collider falls back to its entity's own mesh,
//! as Unity's `MeshCollider` does with a missing mesh reference.

use std::cell::RefCell;
use std::collections::BTreeSet;

use crate::asset::{self, AssetRef, ImportedAsset};
use crate::components::{ColliderComponent, ColliderShape, SharedMesh};
use crate::scene::Scene;

thread_local! {
    /// The `(owner, name)` pairs already warned of: a model spawned twenty times
    /// says it once.
    static WARNED: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
}

/// The collision mesh `name` of the model at `path` (imported as `asset`), with its
/// geometry, or `None`, warned of once per `owner`, when the model has no such
/// sub-object or it holds no triangles.
pub fn collision_mesh(
    path: &str,
    asset: &ImportedAsset,
    name: &str,
    owner: &str,
) -> Option<SharedMesh> {
    let sub = asset.sub_mesh(name).filter(|s| !s.indices.is_empty());
    let Some(sub) = sub.filter(|s| s.bounds().is_some()) else {
        warn_once(
            owner,
            &format!("{path}::{name}"),
            "the model has no such mesh",
        );
        return None;
    };
    Some(SharedMesh {
        primitive_type: "Asset".to_string(),
        asset_ref: Some(format!("{path}{}{name}", asset::REF_SEPARATOR)),
        subdivisions: 1,
        positions: sub.vertices.iter().map(|v| v.position).collect(),
        indices: sub.indices.clone(),
    })
}

/// Say once per `owner` and `reference` that the collider falls back to its own mesh.
fn warn_once(owner: &str, reference: &str, why: &str) {
    let key = format!("{owner} -> {reference}");
    if WARNED.with(|w| w.borrow_mut().insert(key)) {
        log::warn!(
            "[Scene] {owner}'s collision mesh '{reference}' is not used ({why}): \
             its collider builds from its own mesh"
        );
    }
}

/// Make entity `id`'s collider a Mesh shape (`convex`: one hull), built from the
/// collision mesh `mesh` names, or from its own Mesh when `mesh` is `None` or names
/// nothing the model has (warned once). `mesh` is a `path::sub_object` reference, or
/// a bare sub-object of the model the entity's Mesh comes from. An entity without a
/// Mesh keeps the shared mesh it has (the one kept when its Mesh was removed).
/// The one write `Physics.SetColliderShape` makes for a `"Mesh"` shape. Errors when
/// the entity has no collider or nothing to build the shape from.
pub fn set_mesh_shape(
    scene: &mut Scene,
    id: u32,
    convex: bool,
    mesh: Option<&str>,
) -> Result<(), String> {
    let own = scene.world.mesh(id).map(|m| {
        let positions: Vec<[f32; 3]> = m.vertices.iter().map(|v| v.position).collect();
        (positions, m.asset_ref.clone())
    });
    let named = mesh.and_then(|name| named_mesh(scene, id, own.as_ref(), name));
    let Some(mut collider) = scene.world.collider_mut(id) else {
        return Err(format!("entity {id} has no Collider"));
    };
    let shared = match (named, &own) {
        (Some(shared), _) => Some(shared),
        (None, Some(_)) => None,
        (None, None) => collider.shared_mesh.clone(),
    };
    let positions = match (&shared, &own) {
        (Some(s), _) => s.positions.clone(),
        (None, Some((p, _))) => p.clone(),
        (None, None) => Vec::new(),
    };
    let Some((min, max)) = bounds(&positions) else {
        return Err(format!(
            "entity {id}: a Mesh collider needs a Mesh or a collision `mesh` to build from"
        ));
    };
    set_shape_and_mesh(&mut collider, convex, (min, max), shared);
    Ok(())
}

/// The collision mesh `name` resolves to for entity `id`, or `None` (warned once).
fn named_mesh(
    scene: &Scene,
    id: u32,
    own: Option<&(Vec<[f32; 3]>, Option<String>)>,
    name: &str,
) -> Option<SharedMesh> {
    let owner = scene
        .world
        .name(id)
        .map_or_else(|| format!("entity {id}"), |n| n.clone());
    let reference = if name.contains(asset::REF_SEPARATOR) {
        name.to_string()
    } else {
        let model = own.and_then(|(_, r)| AssetRef::parse(r.as_deref()?).ok());
        let Some(model) = model else {
            warn_once(
                &owner,
                name,
                "its Mesh comes from no model to look the name up in",
            );
            return None;
        };
        format!("{}{}{name}", model.path, asset::REF_SEPARATOR)
    };
    let parsed = match AssetRef::parse(&reference) {
        Ok(r) => r,
        Err(e) => {
            warn_once(&owner, &reference, &e.to_string());
            return None;
        }
    };
    match asset::imported(std::path::Path::new(&parsed.path)) {
        Ok(asset) => collision_mesh(&parsed.path, &asset, &parsed.sub_object, &owner),
        Err(e) => {
            warn_once(&owner, &reference, &e.to_string());
            None
        }
    }
}

/// Put a Mesh shape over `(min, max)` and its shared mesh on `collider`.
fn set_shape_and_mesh(
    collider: &mut ColliderComponent,
    convex: bool,
    (local_min, local_max): (glam::Vec3, glam::Vec3),
    shared: Option<SharedMesh>,
) {
    collider.shape = ColliderShape::Mesh {
        convex,
        local_min,
        local_max,
    };
    collider.shared_mesh = shared;
}

/// The box around `positions`, or `None` for none.
pub(super) fn bounds(positions: &[[f32; 3]]) -> Option<(glam::Vec3, glam::Vec3)> {
    let mut points = positions.iter().copied().map(glam::Vec3::from_array);
    let first = points.next()?;
    Some(points.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p))))
}

/// How many fallbacks `owner` was warned of: each is said once.
#[cfg(test)]
pub(super) fn warnings_for(owner: &str) -> usize {
    let prefix = format!("{owner} -> ");
    WARNED.with(|w| w.borrow().iter().filter(|k| k.starts_with(&prefix)).count())
}

#[cfg(test)]
#[path = "collision_tests.rs"]
pub(crate) mod tests;
