use std::collections::BTreeMap;
use std::path::Path;

use mlua::{Lua, Table, Value};
use serde::Deserialize;

use super::{folder, put};
use crate::api::Reg;
use rusty_asset::sidecar::{check_bake_size, prepare_bake};
use rusty_asset::SubMesh;
use rusty_lighting::mesh_maps::{bake, Faces, MeshMap, MeshSource, Settings, MAX_RESOLUTION};
use rusty_procgen::bake::bake_to_png;
use rusty_procgen::image_buf::Size;

/// The square map's edge in texels when a call names none; `docs/api/Texture.md` states it.
pub(super) const DEFAULT_RESOLUTION: u32 = 1024;

pub(super) fn register(lua: &Lua, table: &Table) -> Reg {
    put(
        table,
        "BakeMeshMaps",
        lua.create_function(
            |lua, (model, maps, prefix, options): (String, Vec<String>, String, Value)| {
                let (settings, faces) = settings(&options)
                    .map_err(|e| format!("Texture.BakeMeshMaps: {model}: {e}"))
                    .map_err(mlua::Error::RuntimeError)?;
                let root = rusty_core::project::root();
                let call = (settings, faces);
                let written = bake_mesh_maps(&model, &maps, &prefix, &call, &root)
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
    ao_distance: Option<f32>,
    ao_rays: Option<u32>,
    thickness_rays: Option<u32>,
    material: Option<serde_json::Value>,
    ignore_occluders: Option<serde_json::Value>,
}

/// Material names as the script wrote them, resolved once the model is loaded.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct FaceNames {
    pub material: Option<Vec<String>>,
    pub ignore_occluders: Vec<String>,
}

fn names(field: &str, value: Option<serde_json::Value>) -> Result<Option<Vec<String>>, String> {
    use serde_json::Value as J;
    let bad = || format!("settings: {field} is a material name or a list of them");
    match value {
        None => Ok(None),
        Some(J::String(name)) => Ok(Some(vec![name])),
        // An empty Lua table reaches JSON as an object.
        Some(J::Object(o)) if o.is_empty() => Err(empty(field)),
        Some(J::Array(list)) if list.is_empty() => Err(empty(field)),
        Some(J::Array(list)) => list
            .into_iter()
            .map(|v| v.as_str().map(str::to_string).ok_or_else(bad))
            .collect::<Result<_, _>>()
            .map(Some),
        Some(_) => Err(bad()),
    }
}

fn empty(field: &str) -> String {
    format!("settings: {field} is an empty list; name a material or leave {field} out")
}

pub(super) fn settings(options: &Value) -> Result<(Settings, FaceNames), String> {
    let plain = |s| Ok((s, FaceNames::default()));
    let table = match options {
        Value::Nil => return plain(Settings::new(DEFAULT_RESOLUTION)),
        Value::Integer(n) => return plain(Settings::new(side(*n)?)),
        Value::Number(n) if n.fract() == 0.0 => return plain(Settings::new(side(*n as i64)?)),
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
    let faces = FaceNames {
        material: names("material", o.material)?,
        ignore_occluders: names("ignore_occluders", o.ignore_occluders)?.unwrap_or_default(),
    };
    let settings = Settings {
        curvature_radius: o.curvature_radius,
        ao_distance: o.ao_distance,
        ao_rays: o.ao_rays.unwrap_or(defaults.ao_rays),
        thickness_rays: o.thickness_rays.unwrap_or(defaults.thickness_rays),
        ..defaults
    };
    settings.check()?;
    Ok((settings, faces))
}

fn side(n: i64) -> Result<u32, String> {
    u32::try_from(n).map_err(|_| format!("resolution {n} is outside 1..={MAX_RESOLUTION}"))
}

pub(super) fn bake_mesh_maps(
    model: &str,
    maps: &[String],
    prefix: &str,
    (settings, faces): &(Settings, FaceNames),
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
    let (source, materials) = load(model).map_err(fail)?;
    let settings = &Settings {
        faces: resolve(faces, &materials).map_err(fail)?,
        ..settings.clone()
    };
    let path = |map: &MeshMap| format!("{prefix}{}.png", map.name());
    let paths: Vec<String> = maps.iter().map(path).collect();
    let Size { width, height } = settings.size;
    check_bake_size((width, height)).map_err(fail)?;
    folder::make_parents(&paths[0], root).map_err(fail)?;
    let mut out = BTreeMap::new();
    let notes = bake(&source, &maps, settings, |map, image| {
        // Once the baker has checked the settings and the mesh, before the first map is written.
        if out.is_empty() {
            let all = paths.iter().map(String::as_str);
            prepare_bake("Texture.BakeMeshMaps", (width, height), all)?;
        }
        let path = path(&map);
        bake_to_png(&image, map.slot(), &path)?;
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

fn resolve(names: &FaceNames, materials: &[(usize, String)]) -> Result<Faces, String> {
    let find = |list: &[String]| -> Result<Vec<usize>, String> {
        let mut out = Vec::new();
        for name in list {
            let found = materials.iter().filter(|(_, n)| n == name).map(|&(i, _)| i);
            let before = out.len();
            out.extend(found);
            if out.len() == before {
                let known: Vec<_> = materials.iter().map(|(_, n)| n.as_str()).collect();
                return Err(match known.is_empty() {
                    true => format!("has no material \"{name}\" (it has no material slots)"),
                    false => format!(
                        "has no material \"{name}\" (its materials: {})",
                        known.join(", ")
                    ),
                });
            }
        }
        Ok(out)
    };
    Ok(Faces {
        materials: names.material.as_deref().map(find).transpose()?,
        ignore_occluders: find(&names.ignore_occluders)?,
    })
}

/// The mesh, and the index and name of each material its runs draw with.
fn load(model: &str) -> Result<(MeshSource, Vec<(usize, String)>), String> {
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
    let used = |mesh: &SubMesh| {
        let mut used: Vec<usize> = mesh.ranges.iter().filter_map(|r| r.material).collect();
        used.sort_unstable();
        used.dedup();
        let name = |i: usize| imported.asset.materials.get(i).map(|m| m.name.clone());
        let names = used.into_iter().filter_map(|i| Some((i, name(i)?)));
        (MeshSource::from_sub_mesh(mesh), names.collect())
    };
    pick.map(used).ok_or_else(|| {
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

#[cfg(test)]
#[path = "mesh_maps_faces_tests.rs"]
mod faces_tests;
