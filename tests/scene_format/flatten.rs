//! Structs with a `#[serde(flatten)]` field. serde reads the flattened part from its
//! own buffer, out of the tracer's reach, and never names the struct's fields; so
//! each such struct is listed here: its own fields, traced as usual, and the
//! flattened type, traced on its own at the same place and fed back as JSON.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use rusty::components::particle::ParticleEmitterComponent;
use rusty::components::Playback;
use serde::de::Error as _;
use serde::de::{DeserializeSeed, IntoDeserializer, MapAccess, Visitor};
use serde_json::Value;

use super::trace::{trace, Error, Pass};

type Inner = fn(&str) -> (BTreeSet<String>, Value);

/// The struct's own fields (every one, defaulted or not) and its flattened type.
pub fn registry(name: &str) -> Option<(&'static [&'static str], Inner)> {
    Some(match name {
        "AnimatorComponent" => (
            &[
                "speed",
                "is_playing",
                "freeze",
                "parameters",
                "graph",
                "graph_enabled",
                "layers",
                "ik",
            ],
            inner::<Playback>,
        ),
        "LayerState" => (&["name", "weight"], inner::<Playback>),
        "LegacyEmitter" => (&["size_end"], inner::<ParticleEmitterComponent>),
        _ => return None,
    })
}

thread_local! {
    static TRACED: RefCell<BTreeMap<String, (BTreeSet<String>, Value)>> = RefCell::default();
}

/// `T` traced at `path` (once per path), and its first built value as JSON.
fn inner<T: serde::de::DeserializeOwned + serde::Serialize>(
    path: &str,
) -> (BTreeSet<String>, Value) {
    if let Some(hit) = TRACED.with(|t| t.borrow().get(path).cloned()) {
        return hit;
    }
    let (lines, value) = trace::<T>(path);
    let traced = (
        lines,
        serde_json::to_value(value).expect("a built value serializes"),
    );
    TRACED.with(|t| t.borrow_mut().insert(path.to_string(), traced.clone()));
    traced
}

pub fn visit<'de, V: Visitor<'de>>(
    pass: &mut Pass<'_>,
    name: &str,
    v: V,
) -> Result<V::Value, Error> {
    let (own, inner) = registry(name).unwrap_or_else(|| {
        panic!("{name} has a #[serde(flatten)] field: list it in tests/scene_format/flatten.rs")
    });
    let (lines, sample) = inner(&pass.path);
    pass.lines.extend(lines);
    let Value::Object(flat) = sample else {
        return Err(Error::custom(format!(
            "{name}'s flattened part is not a JSON object"
        )));
    };
    v.visit_map(Flat {
        pass,
        own: own.iter(),
        flat: flat.into_iter(),
        next: None,
    })
}

enum Next {
    Own(&'static str),
    Flat(Value),
}

/// The struct's own fields from the tracer, then the flattened part's from JSON.
struct Flat<'p, 's> {
    pass: &'p mut Pass<'s>,
    own: std::slice::Iter<'static, &'static str>,
    flat: serde_json::map::IntoIter,
    next: Option<Next>,
}

impl<'de> MapAccess<'de> for Flat<'_, '_> {
    type Error = Error;
    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        if let Some(&field) = self.own.next() {
            self.next = Some(Next::Own(field));
            return seed.deserialize(field.into_deserializer()).map(Some);
        }
        let Some((key, value)) = self.flat.next() else {
            return Ok(None);
        };
        self.next = Some(Next::Flat(value));
        seed.deserialize(IntoDeserializer::<Error>::into_deserializer(key))
            .map(Some)
    }
    fn next_value_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<T::Value, Error> {
        match self
            .next
            .take()
            .expect("serde asks for a key before its value")
        {
            Next::Own(field) => self.pass.at(&format!(".{field}"), |p| seed.deserialize(p)),
            Next::Flat(value) => seed.deserialize(value).map_err(Error::custom),
        }
    }
}
