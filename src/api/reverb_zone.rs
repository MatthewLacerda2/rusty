//! src/api/reverb_zone.rs — `AudioReverbZone` namespace (#469).
//!
//! Get/Set over an entity's `ReverbZoneComponent` (Unity's `AudioReverbZone`): the
//! full-effect and fade-out radii, the preset, and the raw params. Every setter
//! routes through the shared `scene::authoring::reverb_zone` ops the inspector card
//! uses, so picking a preset writes its params and setting params makes the zone
//! `Custom`. Getters return `nil` without a zone; setters are then an error naming
//! the entity. The listener's blended result is `Audio.GetReverbState`.

use std::cell::RefCell;

use mlua::{Lua, Table, Value};

use super::{put, Reg};
use crate::components::{ReverbParams, ReverbPreset, ReverbZoneComponent};
use crate::scene::authoring::reverb_zone as ops;
use crate::scene::Scene;

type SceneCell<'s> = &'s RefCell<Scene>;

/// The keys a params table carries.
const KEYS: [&str; 4] = ["decay_time", "pre_delay", "damping", "wet"];

/// Register the `AudioReverbZone` namespace onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_radii(scope, &t, scene)?;
    register_preset(scope, &t, scene)?;
    register_params(scope, &t, scene)?;
    lua.globals()
        .set("AudioReverbZone", t)
        .map_err(|e| e.to_string())
}

fn err(message: String) -> mlua::Error {
    mlua::Error::RuntimeError(message)
}

/// `id`'s zone, cloned, or `None` without one.
fn get(scene: SceneCell, id: u32) -> Option<ReverbZoneComponent> {
    scene.borrow().world.reverb_zone(id).map(|z| z.clone())
}

/// Apply `f` to `id`'s zone; an error without one.
fn set(scene: SceneCell, id: u32, f: impl FnOnce(&mut ReverbZoneComponent)) -> mlua::Result<()> {
    let mut s = scene.borrow_mut();
    let mut z = s
        .world
        .reverb_zone_mut(id)
        .ok_or_else(|| err(format!("entity {id} has no AudioReverbZone")))?;
    f(&mut z);
    Ok(())
}

/// `Get/SetMinDistance`, `Get/SetMaxDistance`.
fn register_radii<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    t: &Table,
    scene: SceneCell<'scope>,
) -> Reg {
    let f = scope.create_function(move |_, id: u32| Ok(get(scene, id).map(|z| z.min_distance)));
    put(t, "GetMinDistance", f)?;
    let f = scope.create_function(move |_, (id, d): (u32, f32)| {
        set(scene, id, |z| ops::set_min_distance(z, d))
    });
    put(t, "SetMinDistance", f)?;
    let f = scope.create_function(move |_, id: u32| Ok(get(scene, id).map(|z| z.max_distance)));
    put(t, "GetMaxDistance", f)?;
    let f = scope.create_function(move |_, (id, d): (u32, f32)| {
        set(scene, id, |z| ops::set_max_distance(z, d))
    });
    put(t, "SetMaxDistance", f)
}

/// `GetPreset`, `SetPreset`, `GetPresets`.
fn register_preset<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    t: &Table,
    scene: SceneCell<'scope>,
) -> Reg {
    let f = scope.create_function(move |_, id: u32| Ok(get(scene, id).map(|z| z.preset.name())));
    put(t, "GetPreset", f)?;
    let f = scope.create_function(move |_, (id, name): (u32, String)| {
        let preset = ReverbPreset::parse(&name).ok_or_else(|| {
            let names: Vec<_> = ReverbPreset::ALL.iter().map(|p| p.name()).collect();
            err(format!(
                "unknown reverb preset '{name}' ({})",
                names.join(", ")
            ))
        })?;
        set(scene, id, |z| ops::set_preset(z, preset))
    });
    put(t, "SetPreset", f)?;
    let f = scope.create_function(|_, ()| Ok(ReverbPreset::ALL.map(|p| p.name()).to_vec()));
    put(t, "GetPresets", f)
}

/// `GetParams`, `SetParams`.
fn register_params<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    t: &Table,
    scene: SceneCell<'scope>,
) -> Reg {
    let f = scope.create_function(move |lua, id: u32| {
        get(scene, id)
            .map(|z| params_table(lua, &z.params))
            .transpose()
    });
    put(t, "GetParams", f)?;
    let f = scope.create_function(move |_, (id, patch): (u32, Table)| {
        let Some(zone) = get(scene, id) else {
            return Err(err(format!("entity {id} has no AudioReverbZone")));
        };
        let params = patched(zone.params, &patch)?;
        set(scene, id, |z| ops::set_params(z, params))
    });
    put(t, "SetParams", f)
}

/// `p` as a Lua table keyed by [`KEYS`].
pub(crate) fn params_table(lua: &Lua, p: &ReverbParams) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("decay_time", p.decay_time)?;
    t.set("pre_delay", p.pre_delay)?;
    t.set("damping", p.damping)?;
    t.set("wet", p.wet)?;
    Ok(t)
}

/// `params` with the fields `patch` names replaced; an unknown key is an error.
fn patched(mut params: ReverbParams, patch: &Table) -> mlua::Result<ReverbParams> {
    for pair in patch.clone().pairs::<String, Value>() {
        let (key, value) = pair?;
        let v = match value {
            Value::Integer(i) => i as f32,
            Value::Number(n) => n as f32,
            _ => return Err(err(format!("reverb '{key}' must be a number"))),
        };
        match key.as_str() {
            "decay_time" => params.decay_time = v,
            "pre_delay" => params.pre_delay = v,
            "damping" => params.damping = v,
            "wet" => params.wet = v,
            _ => {
                let valid = KEYS.join(", ");
                return Err(err(format!("unknown reverb param '{key}' ({valid})")));
            }
        }
    }
    Ok(params)
}
