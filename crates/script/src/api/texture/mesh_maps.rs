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

pub(super) const DEFAULT_RESOLUTION: u32 = 1024;

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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Options {
    resolution: Option<u32>,
    size: Option<[u32; 2]>,
    curvature_radius: Option<f32>,
    ao_rays: Option<u32>,
    thickness_rays: Option<u32>,
}

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

fn side(n: i64) -> Result<u32, String> {
    u32::try_from(n).map_err(|_| format!("resolution {n} is outside 1..={MAX_RESOLUTION}"))
}

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
    let path = |map: &MeshMap| format!("{prefix}{}.png", map.name());
    let paths: Vec<String> = maps.iter().map(path).collect();
    let Size { width, height } = settings.size;
    check_bake_size((width, height)).map_err(fail)?;
    folder::make_parents(&paths[0], root).map_err(fail)?;
    let mut out = BTreeMap::new();
    let notes = bake(&source, &maps, settings, |map, image| {
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
