//! src/api/assets.rs — `Assets` namespace (#179).
//!
//! The "see what I can place" half of authoring: surface the importer's per-file
//! inventory as a project-wide, reference-ready manifest. `Assets.Manifest()` (and
//! its `Assets.List` alias) walks the project asset root, imports every model file,
//! and returns each file's addressable sub-objects with their footprint (AABB) and
//! material count — the same catalogue the content browser shows a human, so the
//! agent gets parity. Every `reference` round-trips through `AssetRef` /
//! `import_sub_mesh`, so the instantiate verb can place exactly what the manifest
//! names. `Assets.Refresh()` is the editor's auto-refresh on demand: it imports what
//! arrived since the last look (MP3 → WAV, #385).
//! `Assets.GetLightmapUVSettings` / `SetLightmapUVSettings` (#831) read and write a
//! model's Generate Lightmap UVs import setting, the model inspector's checkbox.
//! `Assets.GetModelImportSettings` / `SetModelImportSettings` (#1398) read and write
//! a model's Scale Factor, the inspector's drag.
//! `Assets.GetTextureImportSettings` / `SetTextureImportSettings` (#1323) read and
//! write an image's wrap, filter and mipmap settings, the image inspector's card.

use std::cell::RefCell;
use std::path::Path;

use mlua::{Lua, Table};

use super::{global_table, put, Reg};
use crate::asset::audio::{refresh, Refresh};
use crate::asset::sidecar::{FilterMode, TextureSettings, WrapMode};
use crate::asset::{build_manifest, AssetEntry, SubObjectEntry};
use crate::asset::{sidecar, LightmapUvSettings};
use crate::core::paths::to_slash;
use crate::scene::Scene;

/// The project asset root the manifest walks — the open project's `assets/`, the
/// same tree the content browser's Sources panel roots at.
const ASSET_ROOT: &str = crate::core::project::ASSETS_DIR;

/// Register the `Assets` namespace onto `lua`.
pub fn register(lua: &Lua) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "Manifest",
        lua.create_function(|lua, ()| manifest_table(lua)),
    )?;
    // `List` is an alias for `Manifest` — same data, the more discoverable name.
    put(
        &table,
        "List",
        lua.create_function(|lua, ()| manifest_table(lua)),
    )?;

    put(
        &table,
        "Refresh",
        lua.create_function(|lua, ()| refresh_table(lua, &refresh(Path::new(ASSET_ROOT)))),
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
            let stored = sidecar::load(Path::new(&path)).map_err(runtime)?;
            texture_table(lua, &stored.texture)
        }),
    )?;
    put(
        &table,
        "SetTextureImportSettings",
        lua.create_function(|_, (path, t): (String, Table)| {
            let path = Path::new(&path);
            let stored = sidecar::load(path).map_err(runtime)?.texture;
            let s = texture_settings(stored, &t)?;
            sidecar::set_texture(path, s).map_err(runtime)
        }),
    )?;

    lua.globals()
        .set("Assets", table)
        .map_err(|e| e.to_string())
}

/// Add the verbs that borrow the scene onto `Assets`: `SetLightmapUVSettings(path,
/// t)` writes the keys `t` names (the rest keep their value) through the verb the
/// model inspector uses, re-importing the model's meshes in the scene, and returns
/// how many it re-imported. `SetModelImportSettings(path, t)` does the same for the
/// Scale Factor.
pub fn register_scoped<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = global_table(lua, "Assets")?;
    put(
        &table,
        "SetLightmapUVSettings",
        scope.create_function(move |_, (path, t): (String, Table)| {
            let stored = sidecar::load(Path::new(&path)).map_err(runtime)?;
            let mut s = stored.lightmap_uvs;
            s.generate = t.get::<Option<bool>>("generate")?.unwrap_or(s.generate);
            s.hard_angle = t.get::<Option<f32>>("hardAngle")?.unwrap_or(s.hard_angle);
            s.pack_margin = t.get::<Option<f32>>("packMargin")?.unwrap_or(s.pack_margin);
            let mut scene = scene.borrow_mut();
            crate::scene::asset_instance::set_lightmap_uv_settings(&mut scene, &path, s)
                .map_err(mlua::Error::RuntimeError)
        }),
    )?;
    put(
        &table,
        "SetModelImportSettings",
        scope.create_function(move |_, (path, t): (String, Table)| {
            let stored = sidecar::load(Path::new(&path)).map_err(runtime)?;
            let scale = t.get::<Option<f32>>("scaleFactor")?.unwrap_or(stored.scale);
            let mut scene = scene.borrow_mut();
            crate::scene::asset_instance::set_model_scale_factor(&mut scene, &path, scale)
                .map_err(mlua::Error::RuntimeError)
        }),
    )
}

/// `{ generate, hardAngle, packMargin }`.
fn lightmap_uv_table(lua: &Lua, s: &LightmapUvSettings) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.raw_set("generate", s.generate)?;
    t.raw_set("hardAngle", s.hard_angle)?;
    t.raw_set("packMargin", s.pack_margin)?;
    Ok(t)
}

/// `{ scaleFactor }`: the model at `path`'s import settings.
fn model_import_table(lua: &Lua, path: &str) -> mlua::Result<Table> {
    let stored = sidecar::load(Path::new(path)).map_err(runtime)?;
    let t = lua.create_table()?;
    t.raw_set("scaleFactor", stored.scale)?;
    Ok(t)
}

/// `{ wrapMode, filterMode, generateMipmaps }`.
fn texture_table(lua: &Lua, s: &TextureSettings) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.raw_set("wrapMode", s.wrap_mode.name())?;
    t.raw_set("filterMode", s.filter_mode.name())?;
    t.raw_set("generateMipmaps", s.generate_mipmaps)?;
    Ok(t)
}

/// `stored` with the keys `t` names replaced; an unknown mode name is an error.
fn texture_settings(mut s: TextureSettings, t: &Table) -> mlua::Result<TextureSettings> {
    if let Some(name) = t.get::<Option<String>>("wrapMode")? {
        s.wrap_mode = WrapMode::parse(&name).map_err(runtime)?;
    }
    if let Some(name) = t.get::<Option<String>>("filterMode")? {
        s.filter_mode = FilterMode::parse(&name).map_err(runtime)?;
    }
    let mipmaps = t.get::<Option<bool>>("generateMipmaps")?;
    s.generate_mipmaps = mipmaps.unwrap_or(s.generate_mipmaps);
    Ok(s)
}

fn runtime(e: impl ToString) -> mlua::Error {
    mlua::Error::RuntimeError(e.to_string())
}

/// Build the manifest under [`ASSET_ROOT`] and marshal it into a Lua array of
/// per-file tables.
fn manifest_table(lua: &Lua) -> mlua::Result<Table> {
    let manifest = build_manifest(Path::new(ASSET_ROOT));
    let assets = lua.create_table()?;
    for (i, entry) in manifest.assets.iter().enumerate() {
        assets.raw_set(i + 1, asset_entry_table(lua, entry)?)?;
    }
    Ok(assets)
}

/// `{ converted = { wavPath, ... }, skipped = { { path, reason }, ... } }`.
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

/// One file's record: `path`, `materialCount`, and an array of `subObjects`.
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

/// One sub-object's record: `id`, the round-trippable `reference`, its footprint
/// `size` (and `min`/`max` AABB when present), and `materialCount`.
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

/// A `{ x, y, z }` table — the structured form a footprint needs (a scalar triple
/// can't be a nested field the way `Transform.GetPosition`'s multi-return can).
fn vec3_table(lua: &Lua, x: f32, y: f32, z: f32) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.raw_set("x", x)?;
    t.raw_set("y", y)?;
    t.raw_set("z", z)?;
    Ok(t)
}

#[cfg(test)]
#[path = "assets_tests.rs"]
mod tests;
