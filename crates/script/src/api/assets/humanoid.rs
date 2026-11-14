//! `Assets.GetHumanoid` / `Assets.SetHumanoid` (#1747): a model's humanoid
//! bone map and the models whose clips are retargeted onto it, as its sidecar
//! keeps them (`asset::humanoid`).

use std::path::Path;

use mlua::{Lua, Table};

use super::runtime;
use crate::api::{put, ApiCell, Reg};
use rusty_asset::humanoid::{BoneMap, HumanoidSettings, CANONICAL};
use rusty_asset::{imported, sidecar};
use rusty_scene::Scene;

/// `GetHumanoid(path)`: needs no scene.
pub(super) fn register(lua: &Lua, table: &Table) -> Reg {
    put(
        table,
        "GetHumanoid",
        lua.create_function(|lua, path: String| humanoid_table(lua, &path)),
    )
}

/// `SetHumanoid(path, { bones?, clipsFrom? })`: re-imports the scene's clips.
pub(super) fn register_set(lua: &Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetHumanoid",
        lua.create_function(move |lua, (path, t): (String, Table)| {
            let scene = &*scene.get(lua)?;
            let stored = sidecar::load(Path::new(&path)).map_err(runtime)?;
            let settings = merged(stored.humanoid, &t)?;
            let mut scene = scene.borrow_mut();
            rusty_scene::asset_instance::set_humanoid(&mut scene, &path, settings)
                .map_err(mlua::Error::RuntimeError)
        }),
    )
}

/// `stored` with `t`'s keys: each `bones` entry sets its canonical bone (`""`
/// unmaps it, `false` goes back to the guess); `clipsFrom` replaces the list.
fn merged(mut stored: HumanoidSettings, t: &Table) -> mlua::Result<HumanoidSettings> {
    if let Some(bones) = t.get::<Option<Table>>("bones")? {
        for pair in bones.pairs::<String, mlua::Value>() {
            let (canonical, bone) = pair?;
            if !CANONICAL.contains(&canonical.as_str()) {
                return Err(runtime(format!("'{canonical}' is no humanoid bone")));
            }
            match bone {
                mlua::Value::Boolean(false) => {
                    stored.bones.remove(&canonical);
                }
                mlua::Value::String(name) => {
                    stored.bones.insert(canonical, name.to_str()?.to_string());
                }
                other => {
                    let kind = other.type_name();
                    return Err(runtime(format!(
                        "{canonical}: a bone name or false, not {kind}"
                    )));
                }
            }
        }
    }
    if let Some(from) = t.get::<Option<Vec<String>>>("clipsFrom")? {
        stored.clips_from = from;
    }
    Ok(stored)
}

/// `{ bones = { Hips = name, ... }, clipsFrom = { path, ... }, problems = { ... } }`:
/// the map as resolved on the model's first skinned sub-mesh (overrides and
/// guesses together), the sources, and what resolving found wrong.
fn humanoid_table(lua: &Lua, path: &str) -> mlua::Result<Table> {
    let settings = sidecar::load(Path::new(path)).map_err(runtime)?.humanoid;
    let import = imported(Path::new(path)).map_err(runtime)?;
    let skin = import.sub_meshes.iter().find_map(|s| s.skin.as_ref());
    let t = lua.create_table()?;
    let bones = lua.create_table()?;
    let problems = lua.create_table()?;
    match skin {
        Some(skin) => {
            let map = BoneMap::resolve(skin, &settings);
            for (canonical, slot) in CANONICAL.iter().zip(&map.slots) {
                if let Some(slot) = slot {
                    bones.raw_set(*canonical, skin.name(*slot))?;
                }
            }
            for (i, p) in map.problems.iter().enumerate() {
                problems.raw_set(i + 1, p.as_str())?;
            }
        }
        None => problems.raw_set(1, "the model has no skeleton")?,
    }
    t.raw_set("bones", bones)?;
    t.raw_set("clipsFrom", settings.clips_from.clone())?;
    t.raw_set("problems", problems)?;
    Ok(t)
}
