//! crates/script/src/api/texture/mesh_maps.rs — `Texture.BakeMeshMaps` (#1849): bake a model's
//! mesh maps (`lighting::mesh_maps`) to `<prefix><map>.png`, for a recipe's
//! `image` op to read.
//!
//! `model` is an asset reference as `Scene.Instantiate` takes it:
//! `path::SubObject`, or a bare model path when the file holds one mesh. The ID map
//! is an 8-bit PNG (its colours are labels, not values); every other map is a 16-bit
//! `data` PNG, so a gradient read from it never terraces.
//!
//! The fourth argument is the resolution or a settings table (#2260,
//! [`settings`]): `resolution` or `size = {w, h}`, `curvature_radius`, `ao_rays`,
//! `thickness_rays`, each defaulting to what a call naming none bakes. A
//! `curvature_radius` narrower than three of some UV island's texels bakes those
//! islands wider, and the call warns once saying so.

use std::collections::BTreeMap;
use std::path::Path;

use mlua::{Lua, Table, Value};
use serde::Deserialize;

use super::{folder, put};
use crate::api::Reg;
use rusty_asset::sidecar::{check_bake_size, prepare_bake};
use rusty_asset::SubMesh;
use rusty_lighting::mesh_maps::{bake, MeshMap, MeshSource, Settings, MAX_RESOLUTION};
use rusty_procgen::bake::bake_to_png;
use rusty_procgen::image_buf::Size;
use rusty_procgen::Slot;

/// The resolution a call that names none bakes at.
pub(super) const DEFAULT_RESOLUTION: u32 = 1024;

/// Put `BakeMeshMaps` on the `Texture` table.
pub(super) fn register(lua: &Lua, table: &Table) -> Reg {
    put(
        table,
        "BakeMeshMaps",
        lua.create_function(
            |lua, (model, maps, prefix, options): (String, Vec<String>, String, Value)| {
                let settings = settings(&options)
                    .map_err(|e| format!("Texture.BakeMeshMaps: {model}: {e}"))
                    .map_err(mlua::Error::RuntimeError)?;
                let root = rusty_core::project::root();
                let written = bake_mesh_maps(&model, &maps, &prefix, &settings, &root)
                    .map_err(mlua::Error::RuntimeError)?;
                lua.create_table_from(written)
            },
        ),
    )
}

/// The settings table's fields, as a script writes them; every one optional.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Options {
    resolution: Option<u32>,
    size: Option<[u32; 2]>,
    curvature_radius: Option<f32>,
    ao_rays: Option<u32>,
    thickness_rays: Option<u32>,
}

/// The bake settings the fourth argument names: `nil` (the defaults), a number
/// (the square resolution) or a settings table. A table naming both `resolution`
/// and `size`, or a field the table doesn't know, is an error, and so is a `size`
/// side out of range (named as `size`); every other range is the baker's to check.
pub(super) fn settings(options: &Value) -> Result<Settings, String> {
    let table = match options {
        Value::Nil => return Ok(Settings::new(DEFAULT_RESOLUTION)),
        Value::Integer(n) => return Ok(Settings::new(side(*n)?)),
        Value::Number(n) if n.fract() == 0.0 => return Ok(Settings::new(side(*n as i64)?)),
        Value::Table(_) => crate::api::lua_json::lua_to_json(options)?,
        other => {
            return Err(format!(
                "the fourth argument is a resolution or a settings table, got {}",
                other.type_name()
            ))
        }
    };
    let o: Options = serde_json::from_value(table).map_err(|e| format!("settings: {e}"))?;
    let size = match (o.resolution, o.size) {
        (Some(_), Some(_)) => return Err("settings name both resolution and size".into()),
        (_, Some([w, h])) => {
            // A square `size` would reach the baker as a resolution and be refused
            // under that name: the field the script wrote is the one an error names.
            if ![w, h].iter().all(|s| (1..=MAX_RESOLUTION).contains(s)) {
                return Err(format!(
                    "size {{{w}, {h}}} has a side outside 1..={MAX_RESOLUTION}"
                ));
            }
            Size::new(w, h)
        }
        (r, None) => Size::from(r.unwrap_or(DEFAULT_RESOLUTION)),
    };
    let defaults = Settings::new(size);
    Ok(Settings {
        curvature_radius: o.curvature_radius,
        ao_rays: o.ao_rays.unwrap_or(defaults.ao_rays),
        thickness_rays: o.thickness_rays.unwrap_or(defaults.thickness_rays),
        ..defaults
    })
}

/// A resolution as a side, or why it can't be one (the baker checks the range).
fn side(n: i64) -> Result<u32, String> {
    u32::try_from(n).map_err(|_| format!("resolution {n} is outside 1..={MAX_RESOLUTION}"))
}

/// Bake `maps` of `model` as `settings` says and write each to `<prefix><map>.png`
/// as it is baked, the folders above the first made before the bake when inside
/// `root`. Returns map → path; a `curvature_radius` some UV island's texels are too
/// coarse to hold is said once, as a console warning naming the radius each baked
/// at, and so is a UV area so many faces stack on that it costs a trace per face
/// per texel. An error names the verb and the model; a map that
/// failed to write stops the bake, the maps before it stay written.
pub(super) fn bake_mesh_maps(
    model: &str,
    maps: &[String],
    prefix: &str,
    settings: &Settings,
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
    let path = |map: &MeshMap| format!("{prefix}{}.png", map.name());
    let paths: Vec<String> = maps.iter().map(path).collect();
    let Size { width, height } = settings.size;
    check_bake_size((width, height)).map_err(fail)?;
    folder::make_parents(&paths[0], root).map_err(fail)?;
    let mut out = BTreeMap::new();
    let notes = bake(&source, &maps, settings, |map, image| {
        // Readied once the baker has checked the settings and the mesh, before
        // the first map is written.
        if out.is_empty() {
            let all = paths.iter().map(String::as_str);
            prepare_bake("Texture.BakeMeshMaps", (width, height), all)?;
        }
        let path = path(&map);
        let slot = match map {
            MeshMap::Id => Slot::Normal,
            _ => Slot::Data,
        };
        bake_to_png(&image, slot, &path)?;
        out.insert(map.name().to_string(), path);
        Ok(())
    })
    .map_err(fail)?;
    if let Some(raised) = notes.raised {
        log::warn!("[Texture] BakeMeshMaps: {model}: {raised}");
    }
    if let Some(stacked) = notes.stacked {
        log::warn!("[Texture] BakeMeshMaps: {model}: {stacked}");
    }
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
