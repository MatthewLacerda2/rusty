//! src/api/assets.rs — `Assets` namespace (#179).
//!
//! The "see what I can place" half of authoring: surface the importer's per-file
//! inventory as a project-wide, reference-ready manifest. `Assets.Manifest()` (and
//! its `Assets.List` alias) walks the project asset root, imports every model file,
//! and returns each file's addressable sub-objects with their footprint (AABB) and
//! material count — the same catalogue the content browser shows a human, so the
//! agent gets parity. Every `reference` round-trips through `AssetRef` /
//! `import_sub_mesh`, so the instantiate verb can place exactly what the manifest
//! names.

use std::path::Path;

use mlua::{Lua, Table};

use super::{put, Reg};
use crate::asset::{build_manifest, AssetEntry, SubObjectEntry};

/// The project asset root the manifest walks — the same `project` tree the content
/// browser's Sources panel roots at.
const ASSET_ROOT: &str = "project";

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

    lua.globals()
        .set("Assets", table)
        .map_err(|e| e.to_string())
}

/// Build the manifest under [`ASSET_ROOT`] and marshal it into a Lua array of
/// per-file tables.
fn manifest_table(lua: &Lua) -> mlua::Result<Table<'_>> {
    let manifest = build_manifest(Path::new(ASSET_ROOT));
    let assets = lua.create_table()?;
    for (i, entry) in manifest.assets.iter().enumerate() {
        assets.raw_set(i + 1, asset_entry_table(lua, entry)?)?;
    }
    Ok(assets)
}

/// One file's record: `path`, `materialCount`, and an array of `subObjects`.
fn asset_entry_table<'lua>(lua: &'lua Lua, entry: &AssetEntry) -> mlua::Result<Table<'lua>> {
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
fn sub_object_table<'lua>(lua: &'lua Lua, sub: &SubObjectEntry) -> mlua::Result<Table<'lua>> {
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
fn vec3_table(lua: &Lua, x: f32, y: f32, z: f32) -> mlua::Result<Table<'_>> {
    let t = lua.create_table()?;
    t.raw_set("x", x)?;
    t.raw_set("y", y)?;
    t.raw_set("z", z)?;
    Ok(t)
}
