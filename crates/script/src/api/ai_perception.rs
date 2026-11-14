//! crates/script/src/api/ai_perception.rs — `AIPerception` namespace (#1900).
//!
//! Unreal's AI Perception (Unity has none): what an agent with an `AIPerception`
//! knows through sight, hearing and damage, the noises a game reports, and the
//! component's settings. The senses themselves run in `ai::perception`; every
//! setter routes through the shared `scene::authoring::ai::perception` ops the
//! inspector card uses, after refusing a value it would not keep (a negative
//! distance or age, a non-finite one, a NaN angle) with an error. Getters read
//! the component in place and return a neutral value without it; setters are
//! then no-ops. Attach one with `Scene.AddComponent(id, "AIPerception")`.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_ai::perception;
use rusty_components::{AIPerceptionComponent as Perception, Known};
use rusty_scene::authoring::ai::perception as ops;
use rusty_scene::Scene;

const NS: &str = "AIPerception";

/// Register the `AIPerception` namespace onto `lua`.
pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_knowledge(lua, &t, scene)?;
    register_noise(lua, &t, scene)?;
    register_distances(lua, &t, scene)?;
    register_masks(lua, &t, scene)?;
    lua.globals().set(NS, t).map_err(|e| e.to_string())
}

/// A known target as scripts see it: `{ id, sense, x, y, z, age, sensed, tag,
/// amount }`, plus `forgotten` when `forgotten` is set (the callback's `info`).
pub fn known_table(
    lua: &Lua,
    target: Option<u32>,
    k: &Known,
    forgotten: bool,
) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("id", target)?;
    t.set("sense", k.sense.name())?;
    t.set("x", k.position.x)?;
    t.set("y", k.position.y)?;
    t.set("z", k.position.z)?;
    t.set("age", k.age)?;
    t.set("sensed", k.sensed)?;
    t.set("tag", k.tag.as_str())?;
    t.set("amount", k.amount)?;
    if forgotten {
        t.set("forgotten", true)?;
    }
    Ok(t)
}

/// `read` of `id`'s perception, in place, or `None` without one.
fn get<T>(scene: &RefCell<Scene>, id: u32, read: impl FnOnce(&Perception) -> T) -> Option<T> {
    scene.borrow().world.ai_perception(id).map(|p| read(&p))
}

/// Apply a shared op to `id`'s perception, if it has one.
fn write(scene: &RefCell<Scene>, id: u32, op: impl FnOnce(&mut Perception)) {
    if let Some(mut p) = scene.borrow_mut().world.ai_perception_mut(id) {
        op(&mut p);
    }
}

/// `GetKnown`, `Get`, `CanSee` and `Forget`.
fn register_knowledge(lua: &Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        let scene = scene.get(lua)?;
        let s = scene.borrow();
        let out = lua.create_table()?;
        let Some(p) = s.world.ai_perception(id.read()) else {
            return Ok(out);
        };
        let at = s.world.world_position(id.read());
        let mut known: Vec<(f32, u32, &Known)> = p
            .memory
            .known
            .iter()
            .map(|(&target, k)| (at.distance(k.position), target, k))
            .collect();
        known.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (_, target, k) in known {
            out.push(known_table(lua, Some(target), k, false)?)?;
        }
        Ok(out)
    });
    put(t, "GetKnown", f)?;
    let f = lua.create_function(move |lua, (id, target): (EntityArg, EntityArg)| {
        let scene = scene.get(lua)?;
        let s = scene.borrow();
        let p = s.world.ai_perception(id.read());
        let k = p.as_ref().and_then(|p| p.memory.known.get(&target.read()));
        k.map(|k| known_table(lua, Some(target.read()), k, false))
            .transpose()
    });
    put(t, "Get", f)?;
    let f = lua.create_function(move |lua, (id, target): (EntityArg, EntityArg)| {
        let sees = |p: &Perception| p.memory.in_sight.contains(&target.read());
        Ok(get(&*scene.get(lua)?, id.read(), sees).unwrap_or(false))
    });
    put(t, "CanSee", f)?;
    let f = lua.create_function(move |lua, (id, target): (EntityArg, Option<EntityArg>)| {
        let id = id.live(NS, "Forget")?;
        let scene = scene.get(lua)?;
        perception::forget(&mut scene.borrow_mut(), id, target.map(EntityArg::read));
        Ok(())
    });
    put(t, "Forget", f)
}

/// `ReportNoise(x, y, z, loudness [, opts])`: how many perceivers heard it.
fn register_noise(lua: &Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    type Args = (f32, f32, f32, f32, Option<Table>);
    let f = lua.create_function(move |lua, (x, y, z, loudness, opts): Args| {
        let point = Vec3::new(x, y, z);
        if !point.is_finite() || !loudness.is_finite() || loudness < 0.0 {
            return Err(mlua::Error::RuntimeError(format!(
                "AIPerception.ReportNoise: the point and loudness must be finite, \
                 the loudness not negative (got ({x}, {y}, {z}), {loudness})"
            )));
        }
        let (instigator, tag) = match &opts {
            Some(o) => (
                o.get::<Option<EntityArg>>("instigator")?
                    .and_then(EntityArg::get),
                o.get::<Option<String>>("tag")?.unwrap_or_default(),
            ),
            None => (None, String::new()),
        };
        let scene = scene.get(lua)?;
        let mut s = scene.borrow_mut();
        Ok(perception::report_noise(
            &mut s, point, loudness, instigator, &tag,
        ))
    });
    put(t, "ReportNoise", f)
}

/// What a float setter accepts: a distance or an age (finite, not negative), the
/// eye height (finite) or the view angle (any number but NaN; it is clamped).
#[derive(Clone, Copy)]
enum Accepts {
    NonNegative,
    Finite,
    Number,
}

impl Accepts {
    /// `v`, or the error a script sees for it.
    fn check(self, verb: &str, v: f32) -> mlua::Result<f32> {
        let (ok, wants) = match self {
            Accepts::NonNegative => (v.is_finite() && v >= 0.0, "a finite number, not negative"),
            Accepts::Finite => (v.is_finite(), "a finite number"),
            Accepts::Number => (!v.is_nan(), "a number"),
        };
        if ok {
            return Ok(v);
        }
        Err(mlua::Error::RuntimeError(format!(
            "{NS}.{verb}: needs {wants}, got {v}"
        )))
    }
}

/// A float setting: its name, what its setter accepts, its getter and its op.
type Float = (
    &'static str,
    Accepts,
    fn(&Perception) -> f32,
    fn(&mut Perception, f32),
);

/// The distances, the angle, the eye height and the ages.
fn floats() -> [Float; 8] {
    use Accepts::{Finite, NonNegative, Number};
    [
        (
            "SightRange",
            NonNegative,
            |p| p.sight_range,
            ops::set_sight_range,
        ),
        (
            "LoseSightRange",
            NonNegative,
            |p| p.lose_sight_range,
            ops::set_lose_sight_range,
        ),
        (
            "SightAngle",
            Number,
            |p| p.sight_angle,
            ops::set_sight_angle,
        ),
        ("EyeHeight", Finite, |p| p.eye_height, ops::set_eye_height),
        (
            "HearingRange",
            NonNegative,
            |p| p.hearing_range,
            ops::set_hearing_range,
        ),
        (
            "SightMaxAge",
            NonNegative,
            |p| p.sight_max_age,
            ops::set_sight_max_age,
        ),
        (
            "HearingMaxAge",
            NonNegative,
            |p| p.hearing_max_age,
            ops::set_hearing_max_age,
        ),
        (
            "DamageMaxAge",
            NonNegative,
            |p| p.damage_max_age,
            ops::set_damage_max_age,
        ),
    ]
}

/// `Get*` / `Set*` for the distances, the angle, the eye height and the ages.
fn register_distances(lua: &Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    for (name, accepts, read, set) in floats() {
        let f = lua.create_function(move |lua, id: EntityArg| {
            Ok(get(&*scene.get(lua)?, id.read(), read).unwrap_or(0.0))
        });
        put(t, &format!("Get{name}"), f)?;
        let verb = format!("Set{name}");
        let f = lua.create_function(move |lua, (id, v): (EntityArg, f32)| {
            let id = id.live(NS, &verb)?;
            let v = accepts.check(&verb, v)?;
            write(&*scene.get(lua)?, id, |p| set(p, v));
            Ok(())
        });
        put(t, &format!("Set{name}"), f)?;
    }
    Ok(())
}

/// `Get*` / `Set*` for the two layer masks.
fn register_masks(lua: &Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    type Mask = (
        &'static str,
        fn(&Perception) -> u32,
        fn(&mut Perception, u32),
    );
    let masks: [Mask; 2] = [
        ("SightMask", |p| p.sight_mask, ops::set_sight_mask),
        ("BlockingMask", |p| p.blocking_mask, ops::set_blocking_mask),
    ];
    for (name, read, set) in masks {
        let f = lua.create_function(move |lua, id: EntityArg| {
            Ok(get(&*scene.get(lua)?, id.read(), read).unwrap_or(0))
        });
        put(t, &format!("Get{name}"), f)?;
        let verb = format!("Set{name}");
        let f = lua.create_function(move |lua, (id, mask): (EntityArg, i64)| {
            let id = id.live(NS, &verb)?;
            write(&*scene.get(lua)?, id, |p| set(p, mask as u32));
            Ok(())
        });
        put(t, &format!("Set{name}"), f)?;
    }
    Ok(())
}
