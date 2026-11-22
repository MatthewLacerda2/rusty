use glam::Vec3;

use crate::asset_instance::instantiate_imported;
use crate::authoring::rendering::lod_group::MIN_SIZE;
use rusty_asset::lod::LodSet;
use rusty_asset::{Imported, ImportedAsset};
use rusty_components::rendering::lod_group::default_thresholds;
use rusty_components::{LodGroupComponent, LodLevel};
use rusty_scene::Scene;

pub fn instantiate_lod_set(
    scene: &mut Scene,
    path: &str,
    asset: &Imported,
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
    // The children were spawned before they were parented: their collider AABBs are stale.
    scene.update_all_colliders();
    Ok(group)
}

pub fn lod0_extents(scene: &Scene) -> Vec<(u32, f32)> {
    let groups = scene.world.ids_with_lod_group().into_iter();
    groups
        .filter_map(|id| Some((id, mesh_extent(scene, id)?)))
        .collect()
}

/// Measures the meshes, not the sidecar's factor, so the ratio holds whatever factor was stored.
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
        // A size still at its measured extent, floor included, follows the meshes (1e-4 relative).
        let measured = (group.size - was.max(MIN_SIZE)).abs() <= group.size * 1e-4;
        let size = if measured { now } else { group.size * ratio };
        crate::authoring::rendering::lod_group::set_size(&mut group, size);
    }
}

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

pub(crate) fn lod0_size<'a>(asset: &ImportedAsset, lod0: impl Iterator<Item = &'a str>) -> f32 {
    let bounds = lod0.filter_map(|id| asset.sub_mesh(id).and_then(|s| s.bounds()));
    let union = bounds.reduce(|(a0, a1), (b0, b1)| (a0.min(b0), a1.max(b1)));
    // An LOD0 with no bounds sizes at 1 unit; one smaller than `MIN_SIZE` takes `MIN_SIZE`.
    union.map_or(1.0, |(lo, hi)| (hi - lo).max_element().max(MIN_SIZE))
}

#[cfg(test)]
mod tests;
