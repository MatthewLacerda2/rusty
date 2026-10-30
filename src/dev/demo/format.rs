//! The `.demo` file: JSON Lines, one header and then one line per tick that had
//! anything to say (#1581).
//!
//! ```text
//! {"scene":"assets/scenes/main.scene","engine_commit":"8f6ad5f…","seed":…,"sources":{…},"settings":{…},"storage":{…},"size":[1280,720],"time_scale":1.0,"before_play":[…]}
//! {"t":0,"input":{"held":["W"]},"host":[{"eval":"Time.SetScale(0.5)"}]}
//! {"t":1,"dt":0.0172}
//! {"t":119,"digest":"9b1c…"}
//! {"t":240,"input":{"mouse_delta":[1.0,-2.0]},"size":[1920,1080]}
//! {"end":7200,"digest":"41d0…"}
//! ```
//!
//! A tick no line names had the input and `dt` of the tick before and no host
//! act; the first tick's `dt` is measured from the fixed step. No
//! format version (CLAUDE.md, *No backward compatibility*): the header's engine
//! commit is what a replay checks, and it warns when it differs. A float is
//! written as a JSON number when that number reads back to the same bits, and as
//! its bits otherwise ([`Exact`]), so a replay is bit for bit.

use serde::de::Deserializer;
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::delta::InputDelta;
use super::sources::Sources;
use crate::core::project::ProjectSettings;
use crate::scripting::{HostCall, Plain};

/// An `f64` that survives the file bit for bit.
#[derive(Clone, Copy, Debug)]
pub struct Exact(pub f64);

impl PartialEq for Exact {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

/// `x` as a JSON number when that reads back to the same bits, else `None`.
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

/// What a replay needs before its first tick.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Header {
    /// The scene Play started in, relative to the project.
    pub scene: String,
    /// The engine build that recorded it.
    pub engine_commit: String,
    /// Where `Random` started.
    pub seed: u64,
    /// The scene file and the project's scripts as they were: a replay on a
    /// changed project says so.
    #[serde(default)]
    pub sources: Sources,
    /// The project settings the run played under.
    pub settings: ProjectSettings,
    /// The save data (`Storage`) Play started from: the player's options with it.
    pub storage: Value,
    /// The Game view's pixel size when Play started, if a platform had set one.
    pub size: Option<(u32, u32)>,
    /// `Time.timeScale` as edit mode left it: entering Play keeps it.
    pub time_scale: Exact,
    /// `Time.paused` and the queued `Time.Step`s as edit mode left them: entering
    /// Play keeps both, as it keeps the time scale.
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub pending_steps: u32,
    /// Host acts made before the tick that entered Play, run before entering it
    /// (a scenario's set-up, a taint on how Play started), as [`Entry::host`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub before_play: Vec<Value>,
}

/// One tick that had something to say.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    /// The tick, counted from the first after Play was entered (`0`).
    pub t: u64,
    /// The tick's `dt` (an `f32`, widened), when it differs from the tick before.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dt: Option<Exact>,
    #[serde(skip_serializing_if = "InputDelta::is_empty")]
    pub input: InputDelta,
    /// Host acts made before this tick, in order ([`host_json`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub host: Vec<Value>,
    /// The Game view's new size, from this tick on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<(u32, u32)>,
    /// The state digest after this tick (`digest::of`), in hex.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

impl Entry {
    /// Whether the tick has nothing to write.
    pub fn is_empty(&self) -> bool {
        self.input.is_empty()
            && self.dt.is_none()
            && self.host.is_empty()
            && self.size.is_none()
            && self.digest.is_none()
    }
}

/// The last line, written on Stop: how many ticks the demo holds, the host acts
/// made after the last of them, and the state digest Play ended on.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct End {
    pub end: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub host: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

/// A host act as a JSON value: `{"eval": line}`, `{"call": name, "args": […]}` or
/// `{"taint": what}`.
pub fn host_json(call: &HostCall) -> Value {
    match call {
        HostCall::Eval(line) => json!({ "eval": line }),
        HostCall::Call { function, args } => {
            json!({ "call": function, "args": args.iter().map(plain_json).collect::<Vec<_>>() })
        }
        HostCall::Taint(what) => json!({ "taint": what }),
    }
}

/// A host act read back; `None` for a line this build does not know.
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

/// The key a float that a JSON number would not carry exactly is written under.
const FLOAT_BITS: &str = "__f64";

/// A plain Lua value as JSON. A Lua float stays a float (`1.0`, not `1`), so
/// `math.type` reads the same on replay.
fn plain_json(value: &Plain) -> Value {
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
