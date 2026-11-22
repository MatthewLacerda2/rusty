//! Unity's `MeshCollider.sharedMesh` set to a low-poly copy, Unreal's `UCX_` mesh.
//! A name the model lacks is warned once and kept with no geometry: the collider falls back to its
//! entity's own mesh, as Unity's `MeshCollider` does.

use std::path::Path;

use rusty_asset::{AssetRef, ImportedAsset};
use rusty_components::{ColliderShape, SharedMesh};
use rusty_scene::rehydrate::{bounds_of, refit_collider, warn_once_of};
use rusty_scene::Scene;

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

fn reference_only(reference: String) -> SharedMesh {
    SharedMesh {
        primitive_type: "Asset".to_string(),
        asset_ref: Some(reference),
        ..SharedMesh::default()
    }
}

/// Its own key, so a "loads empty" warning said earlier of the same reference does not swallow it.
pub(crate) fn warn_key(reference: &str) -> String {
    format!("collision mesh {reference}")
}

fn warn_unused(reference: &str, why: &str) {
    warn_once_of(&warn_key(reference), || {
        format!("collision mesh '{reference}' is not used ({why}): its collider builds from its own mesh")
    });
}

/// An entity without a collider is left alone, as for every other shape; one with nothing to build
/// the shape from is an error, changing nothing.
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

#[doc(hidden)]
pub mod chair;
#[cfg(test)]
mod edge_tests;
#[cfg(test)]
mod fallback_tests;
#[cfg(test)]
mod tests;
