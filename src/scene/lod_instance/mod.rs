//! src/scene/lod_instance/mod.rs — Instantiate a `_LOD<n>` set as one LODGroup (#472).
//!
//! A model whose sub-objects follow the `_LOD0`, `_LOD1`, … convention
//! (`asset::lod`) is one prop at several levels of detail. Instantiating it builds
//! what Unity's importer builds: a parent entity named after the base carrying an
//! `LODGroup`, one child per level's sub-object (each spawned through the shared
//! [`instantiate_imported`] verb, so it gets its mesh, material and collider exactly as
//! a lone sub-object would), each level listing its children as renderers.
//!
//! `Scene.Instantiate("file.glb::Crate")` spawns a set here, the base name
//! addressing it whole. The whole-model spawn (`scene::model_instance`) groups
//! its LOD nodes itself, per node, and shares `set_levels` and `lod0_size`.

use glam::Vec3;

use crate::asset::lod::LodSet;
use crate::asset::ImportedAsset;
use crate::components::rendering::lod_group::default_thresholds;
use crate::components::{LodGroupComponent, LodLevel};
use crate::scene::asset_instance::instantiate_imported;
use crate::scene::authoring::rendering::lod_group::MIN_SIZE;
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
    let mut levels = Vec::with_capacity(set.levels.len());
    for subs in &set.levels {
        let mut renderers = Vec::with_capacity(subs.len());
        for sub in subs {
            let child = instantiate_imported(scene, path, asset, sub, Some(sub), Vec3::ZERO)?;
            scene.set_parent(child, Some(group))?;
            renderers.push(child);
        }
        levels.push(renderers);
    }
    let lod0 = set.levels.first().into_iter().flatten();
    let size = lod0_size(asset, lod0.map(String::as_str));
    set_levels(scene, group, levels, size);
    // Children spawned before they were parented: their collider AABBs are stale.
    scene.update_all_colliders();
    Ok(group)
}

/// Every LODGroup in `scene` with the largest extent of its LOD0 meshes' combined
/// bounds, as `lod0_size` measures an asset's: what a re-import is compared
/// against ([`rescale_groups_over`]). A group whose LOD0 has no mesh is left out.
pub fn lod0_extents(scene: &Scene) -> Vec<(u32, f32)> {
    let groups = scene.world.ids_with_lod_group().into_iter();
    groups
        .filter_map(|id| Some((id, mesh_extent(scene, id)?)))
        .collect()
}

/// Scale the size of every LODGroup in `scene` with one of `renderers` among its
/// levels by how much its LOD0 meshes grew since `before` ([`lod0_extents`]): their
/// meshes were re-imported at a new Scale Factor (#1398), and the size the renderer
/// measures on screen scales with them. Measuring the meshes, not the sidecar, keeps
/// the ratio true whatever the stored factor was. A size still at its measured
/// extent is re-measured; one the author set by hand is scaled, kept in proportion.
pub fn rescale_groups_over(scene: &mut Scene, renderers: &[u32], before: &[(u32, f32)]) {
    if renderers.is_empty() {
        return;
    }
    for &(id, was) in before {
        let Some(now) = mesh_extent(scene, id) else {
            continue;
        };
        let ratio = now / was;
        let Some(mut group) = scene.world.lod_group_mut(id) else {
            continue;
        };
        let over = group.levels.iter().flat_map(|l| &l.renderers);
        let touched = over.into_iter().any(|r| renderers.contains(r));
        if !touched || !ratio.is_finite() || ratio <= 0.0 || ratio == 1.0 {
            continue;
        }
        // A size still at its measured extent follows it, floor included, so a
        // round trip (1 → 0.0001 → 1) comes back where it started.
        let measured = (group.size - was.max(MIN_SIZE)).abs() <= group.size * 1e-4;
        let size = if measured { now } else { group.size * ratio };
        crate::scene::authoring::rendering::lod_group::set_size(&mut group, size);
    }
}

/// The largest extent of group `id`'s LOD0 meshes' combined local bounds, or `None`
/// when they hold no vertex.
fn mesh_extent(scene: &Scene, id: u32) -> Option<f32> {
    let lod0 = scene.world.lod_group(id)?.levels.first()?.renderers.clone();
    let mut bounds: Option<(Vec3, Vec3)> = None;
    for mesh in lod0.iter().filter_map(|&r| scene.world.mesh(r)) {
        for p in mesh.vertices.iter().map(|v| Vec3::from_array(v.position)) {
            bounds = Some(bounds.map_or((p, p), |(lo, hi)| (lo.min(p), hi.max(p))));
        }
    }
    bounds.map(|(lo, hi)| (hi - lo).max_element())
}

/// Give `group` an `LODGroup` over `levels` (each level's renderers, finest
/// first) at the default thresholds and `size`.
pub(crate) fn set_levels(scene: &mut Scene, group: u32, levels: Vec<Vec<u32>>, size: f32) {
    let heights = default_thresholds(levels.len());
    let levels = levels.into_iter().zip(heights);
    let levels = levels
        .map(|(renderers, screen_height)| LodLevel {
            screen_height,
            renderers,
        })
        .collect();
    scene
        .world
        .set_lod_group(group, Some(LodGroupComponent { levels, size }));
}

/// The largest extent of the LOD0 sub-meshes `lod0`'s combined bounds — the
/// height the renderer measures on screen — floored so a flat or empty level
/// still has a size.
pub(crate) fn lod0_size<'a>(asset: &ImportedAsset, lod0: impl Iterator<Item = &'a str>) -> f32 {
    let bounds = lod0.filter_map(|id| asset.sub_mesh(id).and_then(|s| s.bounds()));
    let union = bounds.reduce(|(a0, a1), (b0, b1)| (a0.min(b0), a1.max(b1)));
    union.map_or(1.0, |(lo, hi)| (hi - lo).max_element().max(MIN_SIZE))
}

#[cfg(test)]
mod tests;
