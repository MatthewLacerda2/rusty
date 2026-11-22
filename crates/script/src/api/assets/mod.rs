use std::path::Path;

use mlua::{Lua, Table};

use super::{global_table, put, Reg};

mod colliders;
mod humanoid;
mod texture;
use crate::api::ApiCell;
use rusty_asset::audio::{refresh, Refresh};
use rusty_asset::{build_manifest, AssetEntry, SubObjectEntry};
use rusty_asset::{sidecar, LightmapUvSettings};
use rusty_core::paths::to_slash;
use rusty_scene::Scene;

const ASSET_ROOT: &str = rusty_core::project::ASSETS_DIR;

pub fn register(lua: &Lua) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    for name in ["Manifest", "List"] {
        put(
            &table,
            name,
            lua.create_function(|lua, ()| manifest_table(lua)),
        )?;
    }

    put(
        &table,
        "Refresh",
        lua.create_function(|lua, ()| refresh_all(lua)),
    )?;

    put(
        &table,
        "GetLightmapUVSettings",
        lua.create_function(|lua, path: String| {
            let stored = sidecar::load(Path::new(&path)).map_err(runtime)?;
            lightmap_uv_table(lua, &stored.lightmap_uvs)
        }),
    )?;

    put(
        &table,
        "GetModelImportSettings",
        lua.create_function(|lua, path: String| model_import_table(lua, &path)),
    )?;

    put(
        &table,
        "GetTextureImportSettings",
        lua.create_function(|lua, path: String| {
            let file = texture::file(&path)?;
            texture::table(lua, &sidecar::read_texture(&file).map_err(runtime)?)
        }),
    )?;
    put(
        &table,
        "SetTextureImportSettings",
        lua.create_function(|_, (path, t): (String, Table)| {
            let file = texture::file(&path)?;
            let s = texture::settings(sidecar::texture(&file), &t)?;
            sidecar::set_texture(&file, s).map_err(runtime)
        }),
    )?;

    humanoid::register(lua, &table)?;
    lua.globals()
        .set("Assets", table)
        .map_err(|e| e.to_string())
}

pub fn register_reimport(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let table = global_table(lua, "Assets")?;
    put(
        &table,
        "SetLightmapUVSettings",
        lua.create_function(move |lua, (path, t): (String, Table)| {
            let scene = &*scene.get(lua)?;
            let stored = sidecar::load(Path::new(&path)).map_err(runtime)?;
            let mut s = stored.lightmap_uvs;
            s.generate = t.get::<Option<bool>>("generate")?.unwrap_or(s.generate);
            s.hard_angle = t.get::<Option<f32>>("hardAngle")?.unwrap_or(s.hard_angle);
            s.pack_margin = t.get::<Option<f32>>("packMargin")?.unwrap_or(s.pack_margin);
            let mut scene = scene.borrow_mut();
            rusty_edit::asset_instance::set_lightmap_uv_settings(&mut scene, &path, s)
                .map_err(mlua::Error::RuntimeError)
        }),
    )?;
    put(
        &table,
        "SetModelImportSettings",
        lua.create_function(move |lua, (path, t): (String, Table)| {
            let scene = &*scene.get(lua)?;
            let mut stored = sidecar::load(Path::new(&path)).map_err(runtime)?;
            let scale = t.get::<Option<f32>>("scaleFactor")?.unwrap_or(stored.scale);
            // Every key is read and checked before the sidecar is written, so a bad call
            // writes nothing.
            if let Some(given) = t.get::<Option<Table>>("colliders")? {
                if colliders::merge(&mut stored.colliders, &given)? {
                    sidecar::save(Path::new(&path), &stored).map_err(runtime)?;
                }
            }
            let mut scene = scene.borrow_mut();
            rusty_edit::asset_instance::set_model_scale_factor(&mut scene, &path, scale)
                .map_err(mlua::Error::RuntimeError)
        }),
    )?;
    humanoid::register_set(lua, &table, scene)
}

fn lightmap_uv_table(lua: &Lua, s: &LightmapUvSettings) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.raw_set("generate", s.generate)?;
    t.raw_set("hardAngle", s.hard_angle)?;
    t.raw_set("packMargin", s.pack_margin)?;
    Ok(t)
}

fn model_import_table(lua: &Lua, path: &str) -> mlua::Result<Table> {
    let stored = sidecar::load(Path::new(path)).map_err(runtime)?;
    let t = lua.create_table()?;
    t.raw_set("scaleFactor", stored.scale)?;
    t.raw_set("colliders", colliders::table(lua, &stored.colliders)?)?;
    Ok(t)
}

fn runtime(e: impl ToString) -> mlua::Error {
    mlua::Error::RuntimeError(e.to_string())
}

fn manifest_table(lua: &Lua) -> mlua::Result<Table> {
    let manifest = build_manifest(Path::new(ASSET_ROOT));
    let assets = lua.create_table()?;
    for (i, entry) in manifest.assets.iter().enumerate() {
        assets.raw_set(i + 1, asset_entry_table(lua, entry)?)?;
    }
    Ok(assets)
}

fn refresh_all(lua: &Lua) -> mlua::Result<Table> {
    let root = Path::new(ASSET_ROOT);
    let t = refresh_table(lua, &refresh(root))?;
    let imported = rusty_asset::import::import_all(root).unwrap_or_default();
    t.raw_set("compressed", imported.textures.encoded)?;
    let failed = lua.create_table()?;
    let images = imported.textures.failed.iter();
    for (i, (file, _)) in imported.failed.iter().chain(images).enumerate() {
        failed.raw_set(i + 1, to_slash(file))?;
    }
    t.raw_set("failed", failed)?;
    Ok(t)
}

fn refresh_table(lua: &Lua, report: &Refresh) -> mlua::Result<Table> {
    let converted = lua.create_table()?;
    for (i, (_, wav)) in report.converted.iter().enumerate() {
        converted.raw_set(i + 1, to_slash(wav))?;
    }
    let skipped = lua.create_table()?;
    for (i, (mp3, why)) in report.skipped.iter().enumerate() {
        let t = lua.create_table()?;
        t.raw_set("path", to_slash(mp3))?;
        t.raw_set("reason", why.as_str())?;
        skipped.raw_set(i + 1, t)?;
    }
    let t = lua.create_table()?;
    t.raw_set("converted", converted)?;
    t.raw_set("skipped", skipped)?;
    Ok(t)
}

fn asset_entry_table(lua: &Lua, entry: &AssetEntry) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.raw_set("path", entry.path.as_str())?;
    t.raw_set("materialCount", entry.material_count)?;
    let subs = lua.create_table()?;
    for (i, sub) in entry.sub_objects.iter().enumerate() {
        subs.raw_set(i + 1, sub_object_table(lua, sub)?)?;
    }
    t.raw_set("subObjects", subs)?;
    Ok(t)
}

fn sub_object_table(lua: &Lua, sub: &SubObjectEntry) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.raw_set("id", sub.id.as_str())?;
    t.raw_set("reference", sub.reference.as_str())?;
    t.raw_set("materialCount", sub.material_count)?;
    let size = sub.size();
    t.raw_set("size", vec3_table(lua, size.x, size.y, size.z)?)?;
    if let Some((min, max)) = sub.bounds {
        t.raw_set("min", vec3_table(lua, min.x, min.y, min.z)?)?;
        t.raw_set("max", vec3_table(lua, max.x, max.y, max.z)?)?;
    }
    Ok(t)
}

fn vec3_table(lua: &Lua, x: f32, y: f32, z: f32) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.raw_set("x", x)?;
    t.raw_set("y", y)?;
    t.raw_set("z", z)?;
    Ok(t)
}

#[cfg(test)]
mod tests;
