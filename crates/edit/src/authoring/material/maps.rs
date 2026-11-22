//! A baked slot fills a map field only where it is empty: a path set by hand wins over the bake.

use std::collections::BTreeMap;
use std::path::Path;

use super::MaterialLibrary;
use rusty_asset::sidecar::{check_bake_size, prepare_bake};
use rusty_asset::MapRef;
use rusty_core::paths::to_slash;
use rusty_core::project::ContentRoots;
use rusty_procgen::{bake_set, check_set, set_paths, Slot};
use rusty_scene::MaterialAsset;

pub const MAPS_DIR: &str = rusty_core::project::TEXTURES_DIR;

pub fn bake_maps(
    asset: &mut MaterialAsset,
    name: &str,
    root: &Path,
    dir: &Path,
) -> Result<(), String> {
    let Some(recipe) = &asset.maps_recipe else {
        return Ok(());
    };
    if name.is_empty() || name.contains(['/', '\\']) || name == ".." {
        return Err(format!(
            "material name {name:?} can't name its baked maps; use a plain name like \"rusty_panel\""
        ));
    }
    let in_error = |e: String| format!("material {name:?} maps: {e}");
    check_set(recipe).map_err(in_error)?;
    let loaded = super::load_recipe(recipe, &ContentRoots::new(root)).map_err(in_error)?;
    // `/`-spelled, `dir` included: the scene file stores `/`.
    let prefix = format!("{}/{name}", to_slash(dir).trim_end_matches('/'));
    let size = loaded.recipe.extent();
    let paths = set_paths(&loaded.recipe, &prefix).map_err(in_error)?;
    let paths = paths.iter().map(String::as_str);
    let extent = (size.width, size.height);
    // The size check precedes the folder, so a refused bake makes none.
    check_bake_size(extent).map_err(in_error)?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    prepare_bake("Material maps", extent, paths).map_err(in_error)?;
    let baked = bake_set(&loaded.recipe, &loaded.sources, &prefix).map_err(in_error)?;
    fill_empty_maps(asset, &baked);
    Ok(())
}

/// Unity's way of iterating a texture's import size: author at 256, ship at 2048.
pub fn rebake_maps(
    materials: &mut MaterialLibrary,
    key: &str,
    resolution: Option<u32>,
    root: &Path,
    dir: &Path,
) -> Result<(), String> {
    let mut asset = materials
        .get(key)
        .ok_or_else(|| format!("no material named {key:?}"))?
        .clone();
    let recipe = asset.maps_recipe.as_mut().ok_or_else(|| {
        format!("material {key:?} has no maps recipe to rebake (define it with `maps`)")
    })?;
    if let Some(resolution) = resolution {
        recipe
            .set_resolution(resolution)
            .map_err(|e| format!("material {key:?} maps: {e}"))?;
    }
    bake_maps(&mut asset, key, root, dir)?;
    materials.insert(key.to_string(), asset);
    Ok(())
}

fn fill_empty_maps(asset: &mut MaterialAsset, baked: &BTreeMap<String, String>) {
    for (slot, path) in baked {
        let Ok(slot) = Slot::parse(slot) else {
            continue;
        };
        for field in slot.material_maps() {
            if let Some(target) = map_field(asset, field) {
                target.get_or_insert_with(|| MapRef::from(path.clone()));
            }
        }
    }
    super::fit_map_slots(asset);
}

fn map_field<'a>(asset: &'a mut MaterialAsset, name: &str) -> Option<&'a mut Option<MapRef>> {
    Some(match name {
        "base_color_map" => &mut asset.base_color_map,
        "metallic_map" => &mut asset.metallic_map,
        "roughness_map" => &mut asset.roughness_map,
        "normal_map" => &mut asset.normal_map,
        "emissive_map" => &mut asset.emissive_map,
        "occlusion_map" => &mut asset.occlusion_map,
        "specular_map" => &mut asset.specular_map,
        "height_map" => &mut asset.height_map,
        _ => return None,
    })
}

#[cfg(test)]
#[path = "maps_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "maps_size_tests.rs"]
mod size_tests;
