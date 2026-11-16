//! crates/edit/src/authoring/material/maps.rs — bake a material's own texture recipe (#409).
//!
//! A material may carry the multi-output texture recipe its maps come from
//! (`MaterialAsset.maps_recipe`, the `maps` key of its document). The recipe is the
//! source of truth; the PNGs are a regenerable cache written to
//! `<dir>/<material>_<slot>.png` by [`procgen::bake_set`](rusty_procgen::bake_set),
//! one evaluation for the whole set.
//!
//! Each baked slot fills the map fields it feeds ([`Slot::material_maps`]) **only
//! where the field is empty**: a path the author set by hand wins over the bake. A
//! rebake writes to the same paths, so the fields it filled before already point at
//! them, and a hand-set path stays put — one rule serves define and rebake alike.
//! A recipe's graph instances and `image` nodes read their files from the project
//! root (an `engine/…` path from the engine's content folder) on every bake
//! ([`super::load_recipe`], #1491, #1744), so a rebake picks up an edited graph or
//! source. The asset keeps the recipe as written, instances and
//! all, never its expansion.

use std::collections::BTreeMap;
use std::path::Path;

use super::MaterialLibrary;
use rusty_asset::sidecar::prepare_bake;
use rusty_asset::MapRef;
use rusty_core::paths::to_slash;
use rusty_core::project::ContentRoots;
use rusty_procgen::{bake_set, check_set, set_paths, Slot};
use rusty_scene::MaterialAsset;

/// Where `Material.DefineAsset` / `Material.Rebake` write baked maps: the project's
/// texture workspace, beside hand-made textures.
pub const MAPS_DIR: &str = rusty_core::project::TEXTURES_DIR;

/// Bake `asset`'s maps recipe (named `name`) into `dir` and fill its empty map
/// fields with the written paths, its `image` nodes reading their files from
/// `root` (the open project's, [`rusty_core::project::root`]) and an `engine/…`
/// path from the engine's ([`rusty_core::project::ContentRoots`]). A material with no
/// recipe is left as is. On error nothing is filled, and the recipe is checked
/// before any file is read or written.
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
    // `/`-spelled, `dir` included: the paths land in the scene file, which stores
    // `/` (`core::paths::to_slash`).
    let prefix = format!("{}/{name}", to_slash(dir).trim_end_matches('/'));
    let size = loaded.recipe.extent();
    let paths = set_paths(&loaded.recipe, &prefix).map_err(in_error)?;
    let paths = paths.iter().map(String::as_str);
    let extent = (size.width, size.height);
    // The folder comes first, for the sidecars; a refused bake leaves none it made.
    let made = !dir.exists();
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    prepare_bake("Material maps", extent, paths).map_err(|e| {
        if made {
            let _ = std::fs::remove_dir(dir);
        }
        in_error(e)
    })?;
    let baked = bake_set(&loaded.recipe, &loaded.sources, &prefix).map_err(in_error)?;
    fill_empty_maps(asset, &baked);
    Ok(())
}

/// Re-bake material `key` from its stored recipe, optionally at a new `resolution`
/// (which the recipe keeps, so the next rebake uses it too; a `size` recipe keeps
/// its aspect with its longer side there, [`rusty_procgen::TextureRecipe::set_resolution`]). Unity's way of iterating
/// a texture's import size: author at 256, ship at 2048. The library is unchanged
/// when the bake fails. `root` and `dir` are [`bake_maps`]'s.
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

/// Point every empty map field a baked slot feeds at that slot's PNG.
fn fill_empty_maps(asset: &mut MaterialAsset, baked: &BTreeMap<String, String>) {
    for (slot, path) in baked {
        let Ok(slot) = Slot::parse(slot) else {
            continue;
        };
        for field in slot.material_maps() {
            // Every key a slot names is a field: `maps_tests` walks `Slot::ALL`.
            if let Some(target) = map_field(asset, field) {
                target.get_or_insert_with(|| MapRef::from(path.clone()));
            }
        }
    }
    super::fit_map_slots(asset);
}

/// The map field `MaterialAsset` keeps under the `DefineAsset` key `name`
/// ([`Slot::material_maps`]' spelling), `None` for a key that is no map field.
fn map_field<'a>(asset: &'a mut MaterialAsset, name: &str) -> Option<&'a mut Option<MapRef>> {
    Some(match name {
        "base_color_map" => &mut asset.base_color_map,
        "metallic_map" => &mut asset.metallic_map,
        "roughness_map" => &mut asset.roughness_map,
        "normal_map" => &mut asset.normal_map,
        "emissive_map" => &mut asset.emissive_map,
        "occlusion_map" => &mut asset.occlusion_map,
        "specular_map" => &mut asset.specular_map,
        _ => return None,
    })
}

#[cfg(test)]
#[path = "maps_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "maps_size_tests.rs"]
mod size_tests;
