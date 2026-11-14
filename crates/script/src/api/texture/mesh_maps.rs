//! crates/script/src/api/texture/mesh_maps.rs — `Texture.BakeMeshMaps` (#1849): bake a model's
//! mesh maps (`lighting::mesh_maps`) to `<prefix><map>.png`, for a recipe's
//! `image` op to read.
//!
//! `model` is an asset reference as `Scene.Instantiate` takes it:
//! `path::SubObject`, or a bare model path when the file holds one mesh. The ID map
//! is an 8-bit PNG (its colours are labels, not values); every other map is a 16-bit
//! `data` PNG, so a gradient read from it never terraces.

use std::collections::BTreeMap;
use std::path::Path;

use mlua::{Lua, Table};

use super::{folder, put};
use crate::api::Reg;
use rusty_asset::SubMesh;
use rusty_lighting::mesh_maps::{bake, MeshMap, MeshSource};
use rusty_procgen::bake::bake_to_png;
use rusty_procgen::Slot;

/// The resolution a call that names none bakes at.
pub(super) const DEFAULT_RESOLUTION: u32 = 1024;

/// Put `BakeMeshMaps` on the `Texture` table.
pub(super) fn register(lua: &Lua, table: &Table) -> Reg {
    put(
        table,
        "BakeMeshMaps",
        lua.create_function(
            |lua, (model, maps, prefix, size): (String, Vec<String>, String, Option<u32>)| {
                let size = size.unwrap_or(DEFAULT_RESOLUTION);
                let root = rusty_core::project::root();
                let written = bake_mesh_maps(&model, &maps, &prefix, size, &root)
                    .map_err(mlua::Error::RuntimeError)?;
                lua.create_table_from(written)
            },
        ),
    )
}

/// Bake `maps` of `model` at `resolution`² and write each to `<prefix><map>.png`
/// as it is baked, the folders above the first made before the bake when inside
/// `root`. Returns map → path. An error names the verb and the model; a map that
/// failed to write stops the bake, the maps before it stay written.
pub(super) fn bake_mesh_maps(
    model: &str,
    maps: &[String],
    prefix: &str,
    resolution: u32,
    root: &Path,
) -> Result<BTreeMap<String, String>, String> {
    let fail = |e: String| format!("Texture.BakeMeshMaps: {model}: {e}");
    if maps.is_empty() {
        return Err(fail("no maps asked for".into()));
    }
    let maps = maps
        .iter()
        .map(|m| MeshMap::parse(m))
        .collect::<Result<Vec<_>, _>>()
        .map_err(fail)?;
    let source = load(model).map_err(fail)?;
    // `<prefix>` may end in a folder of its own (`maps/`): make it before the bake.
    folder::make_parents(&format!("{prefix}{}.png", maps[0].name()), root).map_err(fail)?;
    let mut out = BTreeMap::new();
    bake(&source, &maps, resolution, |map, image| {
        let path = format!("{prefix}{}.png", map.name());
        let slot = match map {
            MeshMap::Id => Slot::Normal,
            _ => Slot::Data,
        };
        bake_to_png(&image, slot, &path)?;
        out.insert(map.name().to_string(), path);
        Ok(())
    })
    .map_err(fail)?;
    Ok(out)
}

/// The mesh `model` names, imported.
fn load(model: &str) -> Result<MeshSource, String> {
    let (path, sub) = match model.split_once(rusty_asset::REF_SEPARATOR) {
        Some(_) => {
            let r = rusty_asset::AssetRef::parse(model).map_err(|e| e.to_string())?;
            (r.path, Some(r.sub_object))
        }
        None => (model.to_string(), None),
    };
    let imported =
        rusty_asset::import_and_sync_sidecar(Path::new(&path)).map_err(|e| e.to_string())?;
    let meshes = &imported.asset.sub_meshes;
    let pick: Option<&SubMesh> = match &sub {
        Some(s) => imported.asset.sub_mesh(s),
        None if meshes.len() == 1 => meshes.first(),
        None => None,
    };
    pick.map(MeshSource::from_sub_mesh).ok_or_else(|| {
        let ids = imported.asset.sub_mesh_ids().join(", ");
        match sub {
            Some(s) => format!("has no sub-object \"{s}\" (it has: {ids})"),
            None => format!(
                "holds {} meshes; name one as path::Name ({ids})",
                meshes.len()
            ),
        }
    })
}

#[cfg(test)]
#[path = "mesh_maps_tests.rs"]
mod tests;
