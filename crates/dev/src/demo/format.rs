use serde::de::Deserializer;
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::delta::InputDelta;
use super::sources::Sources;
use rusty_app::GameWorld;
use rusty_core::project::ProjectSettings;
use rusty_script::scripting::{HostCall, Plain};

/// An `f64` kept bit for bit: a JSON number when that reads back to the same bits, else
/// its bits in hex.
#[derive(Clone, Copy, Debug)]
pub struct Exact(pub f64);

impl PartialEq for Exact {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

fn as_number(x: f64) -> Option<Value> {
    let number = serde_json::Number::from_f64(x)?;
    let back: f64 = serde_json::from_str(&number.to_string()).ok()?;
    (back.to_bits() == x.to_bits()).then_some(Value::Number(number))
}

fn to_bits_text(x: f64) -> String {
    format!("{:016x}", x.to_bits())
}

fn from_bits_text(text: &str) -> Option<f64> {
    u64::from_str_radix(text, 16).ok().map(f64::from_bits)
}

impl Serialize for Exact {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match as_number(self.0) {
            Some(number) => number.serialize(s),
            None => to_bits_text(self.0).serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for Exact {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let bad = || serde::de::Error::custom("a number, or a float's bits in hex");
        match Value::deserialize(d)? {
            Value::Number(n) => n.as_f64().map(Exact).ok_or_else(bad),
            Value::String(bits) => from_bits_text(&bits).map(Exact).ok_or_else(bad),
            _ => Err(bad()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Header {
    pub scene: String,
    pub engine_commit: String,
    pub seed: u64,
    #[serde(default)]
    pub sources: Sources,
    pub settings: ProjectSettings,
    pub storage: Value,
    pub size: Option<(u32, u32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_scale: Option<f32>,
    pub time_scale: Exact,
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub pending_steps: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub before_play: Vec<Value>,
}

impl Header {
    pub fn of(world: &GameWorld, scene: String, before_play: &[HostCall]) -> Self {
        let time = world.resources.time.borrow().clone();
        Self {
            scene,
            engine_commit: rusty_core::project::engine_commit().to_string(),
            seed: world.script_manager().play_seed(),
            sources: Sources::of(world),
            settings: world.resources.application.borrow().settings(),
            storage: world.resources.storage.borrow().data(),
            size: world.resources.screen.borrow().game_view(),
            display_scale: world.resources.screen.borrow().reported_display_scale(),
            time_scale: Exact(f64::from(time.time_scale)),
            paused: time.paused,
            pending_steps: time.pending_steps,
            before_play: before_play.iter().map(host_json).collect(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    /// The tick, counted from the first after Play was entered (`0`).
    pub t: u64,
    /// Seconds (an `f32`, widened), written only when it differs from the tick before.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dt: Option<Exact>,
    #[serde(skip_serializing_if = "InputDelta::is_empty")]
    pub input: InputDelta,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub host: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<(u32, u32)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_scale: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loads: Vec<u32>,
    /// The pending load's progress in percent, when it moved at this tick's head.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_progress: Option<u8>,
}

impl Entry {
    pub fn is_empty(&self) -> bool {
        self.input.is_empty()
            && self.dt.is_none()
            && self.host.is_empty()
            && self.size.is_none()
            && self.display_scale.is_none()
            && self.digest.is_none()
            && self.loads.is_empty()
            && self.load_progress.is_none()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct End {
    pub end: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub host: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

pub fn host_json(call: &HostCall) -> Value {
    match call {
        HostCall::Eval(line) => json!({ "eval": line }),
        HostCall::Call { function, args } => {
            json!({ "call": function, "args": args.iter().map(plain_json).collect::<Vec<_>>() })
        }
        HostCall::Taint(what) => json!({ "taint": what }),
    }
}

pub fn host_call(value: &Value) -> Option<HostCall> {
    let text = |key: &str| value.get(key)?.as_str().map(str::to_string);
    if let Some(line) = text("eval") {
        return Some(HostCall::Eval(line));
    }
    if let Some(what) = text("taint") {
        return Some(HostCall::Taint(what));
    }
    let function = text("call")?;
    let args = value
        .get("args")?
        .as_array()?
        .iter()
        .map(json_plain)
        .collect();
    Some(HostCall::Call { function, args })
}

/// The key a float no JSON number carries exactly (a NaN, an infinity) is written under.
const FLOAT_BITS: &str = "__f64";

pub(super) fn plain_json(value: &Plain) -> Value {
    match value {
        Plain::Nil => Value::Null,
        Plain::Bool(b) => Value::Bool(*b),
        Plain::Int(i) => Value::from(*i),
        Plain::Number(x) => match as_number(*x) {
            Some(number) => number,
            None => json!({ FLOAT_BITS: to_bits_text(*x) }),
        },
        Plain::Str(s) => Value::String(s.clone()),
        Plain::List(items) => Value::Array(items.iter().map(plain_json).collect()),
        Plain::Map(pairs) => Value::Object(
            pairs
                .iter()
                .map(|(k, v)| (k.clone(), plain_json(v)))
                .collect(),
        ),
    }
}

fn json_plain(value: &Value) -> Plain {
    match value {
        Value::Null => Plain::Nil,
        Value::Bool(b) => Plain::Bool(*b),
        Value::Number(n) => match n.as_i64() {
            Some(i) => Plain::Int(i),
            None => Plain::Number(n.as_f64().unwrap_or(0.0)),
        },
        Value::String(s) => Plain::Str(s.clone()),
        Value::Array(items) => Plain::List(items.iter().map(json_plain).collect()),
        Value::Object(map) => {
            let bits = map.get(FLOAT_BITS).and_then(Value::as_str);
            match bits.and_then(from_bits_text).filter(|_| map.len() == 1) {
                Some(x) => Plain::Number(x),
                None => Plain::Map(
                    map.iter()
                        .map(|(k, v)| (k.clone(), json_plain(v)))
                        .collect(),
                ),
            }
        }
    }
}

#[cfg(test)]
#[path = "format_tests.rs"]
mod tests;
