//! Reading one tween out of Lua (#424): `Tween.To`'s arguments, or one
//! `Tween.Sequence` item, validated in full before anything is scheduled.

use glam::Vec4;
use mlua::{Lua, Table, Value};

use super::super::timer::{require_active, runtime};
use crate::scene::Scene;
use crate::scripting::tween::{Ease, Property, Tween};
use crate::scripting::Clock;

/// The `opts` keys `Tween.To` accepts; anything else is a typo, and an error.
const OPTS: [&str; 7] = [
    "ease",
    "delay",
    "loops",
    "yoyo",
    "unscaled",
    "from",
    "on_complete",
];

/// A validated tween, ready to schedule.
pub(super) struct Spec {
    pub(super) owner: u32,
    pub(super) tween: Tween,
    /// Seconds before it starts (its `delay`, plus its place in a sequence).
    pub(super) delay: f64,
    pub(super) clock: Clock,
    /// A sequence item that starts with the previous one.
    pub(super) join: bool,
}

impl Spec {
    /// Seconds from its sequence start to its end, or `None` when it loops forever.
    pub(super) fn span(&self) -> Option<f64> {
        let loops = f64::from(self.tween.loops?);
        Some(self.delay + self.tween.duration * loops)
    }
}

/// The positional part of a tween: `(id, property, target, duration)`.
pub(super) struct Args<'lua> {
    pub(super) id: u32,
    pub(super) path: String,
    pub(super) target: Value<'lua>,
    pub(super) duration: f64,
}

/// Validate `args` + `opts` against the live scene. `in_sequence` also accepts a
/// sequence item's own keys (its positional `1..=4` and `join`).
pub(super) fn parse<'lua>(
    lua: &'lua Lua,
    scene: &Scene,
    args: Args<'lua>,
    opts: Option<Table<'lua>>,
    in_sequence: bool,
) -> mlua::Result<Spec> {
    let Args {
        id,
        path,
        target,
        duration,
    } = args;
    require_active(scene, id)?;
    let property = Property::parse(&path).ok_or_else(|| {
        runtime(&format!(
            "unknown tween property `{path}`; animatable: {}",
            Property::paths()
        ))
    })?;
    if property.get(scene, id).is_none() {
        let component = path.split('.').next().unwrap_or_default();
        return Err(runtime(&format!("entity {id} has no {component}")));
    }
    if !(duration.is_finite() && duration >= 0.0) {
        return Err(runtime("tween duration must be a number >= 0"));
    }
    let mut spec = Spec {
        owner: id,
        tween: Tween {
            property,
            from: None,
            to: to_vec4(property, target)?,
            duration,
            ease: Ease::Linear,
            loops: Some(1),
            yoyo: false,
            on_complete: None,
            group: None,
        },
        delay: 0.0,
        clock: Clock::Scaled,
        join: false,
    };
    if let Some(opts) = opts {
        read_opts(lua, &opts, &mut spec, in_sequence)?;
    }
    if spec.tween.loops.is_none() && duration <= 0.0 {
        return Err(runtime(
            "an endless tween (loops = -1) needs a duration > 0",
        ));
    }
    Ok(spec)
}

fn read_opts<'lua>(
    lua: &'lua Lua,
    opts: &Table<'lua>,
    spec: &mut Spec,
    in_sequence: bool,
) -> mlua::Result<()> {
    for pair in opts.clone().pairs::<Value, Value>() {
        let (key, value) = pair?;
        let positional = matches!(key, Value::Integer(1..=4)) && in_sequence;
        let Value::String(key) = key else {
            if positional {
                continue;
            }
            return Err(runtime("tween options are named fields"));
        };
        let key = key.to_str()?;
        let allowed = OPTS.contains(&key) || (in_sequence && key == "join");
        if !allowed {
            return Err(runtime(&format!(
                "unknown tween option `{key}`; options: {}",
                OPTS.join(", ")
            )));
        }
        read_opt(lua, key, value, spec)?;
    }
    Ok(())
}

fn read_opt<'lua>(
    lua: &'lua Lua,
    key: &str,
    value: Value<'lua>,
    spec: &mut Spec,
) -> mlua::Result<()> {
    let t = &mut spec.tween;
    match key {
        "ease" => {
            let name: String = lua.unpack(value)?;
            t.ease = Ease::parse(&name).ok_or_else(|| {
                runtime(&format!("unknown ease `{name}`; eases: {}", Ease::names()))
            })?;
        }
        "delay" => {
            let delay: f64 = lua.unpack(value)?;
            if !(delay.is_finite() && delay >= 0.0) {
                return Err(runtime("tween delay must be a number >= 0"));
            }
            spec.delay = delay;
        }
        "loops" => {
            t.loops = match lua.unpack::<i64>(value)? {
                -1 => None,
                n @ 1.. => Some(u32::try_from(n).map_err(|_| runtime("too many loops"))?),
                _ => return Err(runtime("tween loops must be >= 1, or -1 for endless")),
            };
        }
        "yoyo" => t.yoyo = lua.unpack(value)?,
        "unscaled" => {
            let unscaled: bool = lua.unpack(value)?;
            spec.clock = if unscaled {
                Clock::Unscaled
            } else {
                Clock::Scaled
            };
        }
        "from" => t.from = Some(to_vec4(t.property, value)?),
        "on_complete" => {
            let f: mlua::Function = lua.unpack(value)?;
            t.on_complete = Some(lua.create_registry_value(f)?);
        }
        _ => spec.join = lua.unpack(value)?,
    }
    Ok(())
}

/// A tween value: a number for a one-number property, else a `{x, y[, z[, w]]}`
/// array with exactly as many numbers as the property has.
fn to_vec4(property: Property, value: Value) -> mlua::Result<Vec4> {
    let arity = property.arity();
    let nums: Vec<f32> = match value {
        Value::Integer(n) => vec![n as f32],
        Value::Number(n) => vec![n as f32],
        Value::Table(t) => t.sequence_values::<f32>().collect::<mlua::Result<_>>()?,
        _ => Vec::new(),
    };
    if nums.len() != arity {
        let shape = if arity == 1 {
            "a number".to_string()
        } else {
            format!("a table of {arity} numbers")
        };
        return Err(runtime(&format!("`{}` takes {shape}", property.path())));
    }
    let mut v = Vec4::ZERO;
    for (i, n) in nums.into_iter().enumerate() {
        v[i] = n;
    }
    Ok(v)
}
