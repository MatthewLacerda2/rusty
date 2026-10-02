//! src/scene/authoring/material/maps.rs — bake a material's own texture recipe (#409).
//!
//! A material may carry the multi-output texture recipe its maps come from
//! (`MaterialAsset.maps_recipe`, the `maps` key of its document). The recipe is the
//! source of truth; the PNGs are a regenerable cache written to
//! `<dir>/<material>_<slot>.png` by [`procgen::bake_set`](crate::procgen::bake_set),
//! one evaluation for the whole set.
//!
//! Each baked slot fills the map fields it feeds ([`Slot::material_maps`]) **only
//! where the field is empty**: a path the author set by hand wins over the bake. A
//! rebake writes to the same paths, so the fields it filled before already point at
//! them, and a hand-set path stays put — one rule serves define and rebake alike.

use std::collections::BTreeMap;
use std::path::Path;

use super::MaterialLibrary;
use crate::procgen::{bake_set, Slot};
use crate::scene::MaterialAsset;

/// Where `Material.DefineAsset` / `Material.Rebake` write baked maps: the project's
/// texture workspace, beside hand-made textures.
pub const MAPS_DIR: &str = "project/assets/textures";

/// Bake `asset`'s maps recipe (named `name`) into `dir` and fill its empty map
/// fields with the written paths. A material with no recipe is left as is. On
/// error nothing is filled, and the recipe is checked before any file is written.
pub fn bake_maps(asset: &mut MaterialAsset, name: &str, dir: &Path) -> Result<(), String> {
    let Some(recipe) = &asset.maps_recipe else {
        return Ok(());
    };
    if name.is_empty() || name.contains(['/', '\\']) || name == ".." {
        return Err(format!(
            "material name {name:?} can't name its baked maps; use a plain name like \"rusty_panel\""
        ));
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let prefix = dir.join(name);
    let baked = bake_set(recipe, &prefix.to_string_lossy())
        .map_err(|e| format!("material {name:?} maps: {e}"))?;
    fill_empty_maps(asset, &baked);
    Ok(())
}

/// Re-bake material `key` from its stored recipe, optionally at a new `resolution`
/// (which the recipe keeps, so the next rebake uses it too). Unity's way of iterating
/// a texture's import size: author at 256, ship at 2048. The library is unchanged
/// when the bake fails.
pub fn rebake_maps(
    materials: &mut MaterialLibrary,
    key: &str,
    resolution: Option<u32>,
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
        recipe.resolution = resolution;
    }
    bake_maps(&mut asset, key, dir)?;
    materials.insert(key.to_string(), asset);
    Ok(())
}

/// Point every empty map field a baked slot feeds at that slot's PNG.
fn fill_empty_maps(asset: &mut MaterialAsset, baked: &BTreeMap<String, String>) {
    for (slot, path) in baked {
        let Ok(slot) = Slot::parse(slot) else {
            continue;
        };
        for field in slot.material_maps() {
            let target = match *field {
                "base_color_map" => &mut asset.base_color_map,
                "metallic_map" => &mut asset.metallic_map,
                "roughness_map" => &mut asset.roughness_map,
                "normal_map" => &mut asset.normal_map,
                "emissive_map" => &mut asset.emissive_map,
                _ => continue,
            };
            target.get_or_insert_with(|| path.clone());
        }
    }
}

#[cfg(test)]
#[path = "maps_tests.rs"]
mod tests;
