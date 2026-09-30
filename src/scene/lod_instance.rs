//! src/scene/lod_instance.rs — Instantiate a `_LOD<n>` set as one LODGroup (#472).
//!
//! A model whose sub-objects follow the `_LOD0`, `_LOD1`, … convention
//! (`asset::lod`) is one prop at several levels of detail. Instantiating it builds
//! what Unity's importer builds: a parent entity named after the base carrying an
//! `LODGroup`, one child per level's sub-object (each spawned through the shared
//! [`instantiate_asset`] verb, so it gets its mesh, material and collider exactly as
//! a lone sub-object would), each level listing its children as renderers.
//!
//! Two entry points, one per caller shape: [`instantiate_lod_set`] behind
//! `Scene.Instantiate("file.glb::Crate")` — the base name addresses the whole set —
//! and [`instantiate_model`] for the editor's and preview's "whole file" spawn,
//! which groups each set and spawns every other sub-object on its own.

use glam::Vec3;

use crate::asset::lod::{lod_sets, LodSet};
use crate::asset::{ImportedAsset, REF_SEPARATOR};
use crate::components::lod_group::default_thresholds;
use crate::components::{LodGroupComponent, LodLevel};
use crate::scene::authoring::instantiate_asset;
use crate::scene::authoring::lod_group::MIN_SIZE;
use crate::scene::Scene;

/// Spawn `set` (from the file at `path`) as an LODGroup entity at `position`, named
/// `name` or the set's base. Levels get the default thresholds and the group's size
/// is LOD0's largest extent. Returns the group's id.
pub fn instantiate_lod_set(
    scene: &mut Scene,
    path: &str,
    asset: &ImportedAsset,
    set: &LodSet,
    name: Option<&str>,
    position: Vec3,
) -> Result<u32, String> {
    let group = scene.add_entity(name.unwrap_or(&set.base).to_string());
    if let Some(mut t) = scene.world.transform_mut(group) {
        t.position = position;
    }
    let heights = default_thresholds(set.levels.len());
    let mut levels = Vec::with_capacity(set.levels.len());
    for (subs, screen_height) in set.levels.iter().zip(heights) {
        let mut renderers = Vec::with_capacity(subs.len());
        for sub in subs {
            let reference = format!("{path}{REF_SEPARATOR}{sub}");
            let child = instantiate_asset(scene, &reference, Some(sub), Vec3::ZERO)?;
            scene.set_parent(child, Some(group))?;
            renderers.push(child);
        }
        levels.push(LodLevel {
            screen_height,
            renderers,
        });
    }
    let size = lod0_size(asset, set);
    scene
        .world
        .set_lod_group(group, Some(LodGroupComponent { levels, size }));
    // Children spawned before they were parented: their collider AABBs are stale.
    scene.update_all_colliders();
    Ok(group)
}

/// Spawn every sub-object of the file at `path` at `position`: each LOD set as one
/// LODGroup, every other sub-object as its own entity. Returns the new top-level ids
/// in file order (sets where their first member sits).
pub fn instantiate_model(
    scene: &mut Scene,
    path: &str,
    asset: &ImportedAsset,
    position: Vec3,
) -> Vec<u32> {
    let sets = lod_sets(asset);
    let mut spawned_sets = vec![false; sets.len()];
    let mut ids = Vec::new();
    for sub in &asset.sub_meshes {
        let member = sets
            .iter()
            .position(|s| s.levels.iter().flatten().any(|id| *id == sub.id));
        let id = match member {
            Some(i) if spawned_sets[i] => continue,
            Some(i) => {
                spawned_sets[i] = true;
                instantiate_lod_set(scene, path, asset, &sets[i], None, position)
            }
            None => {
                let reference = format!("{path}{REF_SEPARATOR}{}", sub.id);
                instantiate_asset(scene, &reference, Some(&sub.id), position)
            }
        };
        ids.extend(id.ok());
    }
    ids
}

/// The largest extent of LOD0's combined bounds — the height the renderer measures
/// on screen — floored so a flat or empty level still has a size.
fn lod0_size(asset: &ImportedAsset, set: &LodSet) -> f32 {
    let bounds = set.levels.first().into_iter().flatten();
    let bounds = bounds.filter_map(|id| asset.sub_mesh(id).and_then(|s| s.bounds()));
    let union = bounds.reduce(|(a0, a1), (b0, b1)| (a0.min(b0), a1.max(b1)));
    union.map_or(1.0, |(lo, hi)| (hi - lo).max_element().max(MIN_SIZE))
}

#[cfg(test)]
#[path = "lod_instance_tests.rs"]
mod lod_instance_tests;
