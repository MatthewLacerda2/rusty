//! How a song's written forms are told apart when they are read — by hand, rather than by
//! `#[serde(untagged)]`, so a refusal still names what is wrong.

use std::fmt;
use std::marker::PhantomData;

use serde::de::value::{MapAccessDeserializer, MapDeserializer};
use serde::de::{Error, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use super::{
    ArrangementEntry, Chord, DegreeNote, Layer, Layers, Note, PatchRef, PatternEntry, Play, Steps,
};
use crate::patch::Patch;

enum NameOr<T> {
    Name(String),
    Body(T),
}

struct NameOrVisitor<T> {
    what: &'static str,
    body: PhantomData<T>,
}

impl<'de, T: Deserialize<'de>> Visitor<'de> for NameOrVisitor<T> {
    type Value = NameOr<T>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.what)
    }

    fn visit_str<E: Error>(self, name: &str) -> Result<Self::Value, E> {
        Ok(NameOr::Name(name.to_owned()))
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        T::deserialize(MapAccessDeserializer::new(map)).map(NameOr::Body)
    }
}

fn name_or<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
    what: &'static str,
) -> Result<NameOr<T>, D::Error> {
    deserializer.deserialize_any(NameOrVisitor {
        what,
        body: PhantomData,
    })
}

impl<'de> Deserialize<'de> for PatchRef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(
            match name_or::<D, Patch>(deserializer, "a patch's name, or the patch itself")? {
                NameOr::Name(name) => Self::Named(name),
                NameOr::Body(patch) => Self::Inline(Box::new(patch)),
            },
        )
    }
}

impl<'de> Deserialize<'de> for Layer {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(
            match name_or::<D, Play>(deserializer, "a pattern's name, or a `play` object")? {
                NameOr::Name(name) => Self::Name(name),
                NameOr::Body(play) => Self::Transformed(play),
            },
        )
    }
}

struct Fields;

impl<'de> Visitor<'de> for Fields {
    type Value = Vec<(String, Value)>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut fields = Vec::new();
        while let Some(field) = map.next_entry()? {
            fields.push(field);
        }
        Ok(fields)
    }
}

enum EntryBody {
    Layered(Layers),
    Played(Play),
}

impl<'de> Deserialize<'de> for EntryBody {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let fields = deserializer.deserialize_map(Fields)?;
        let layered = fields.iter().any(|(name, _)| name == "layers");
        let body = MapDeserializer::<_, serde_json::Error>::new(fields.into_iter());
        if layered {
            Layers::deserialize(body).map(Self::Layered)
        } else {
            Play::deserialize(body).map(Self::Played)
        }
        .map_err(D::Error::custom)
    }
}

impl<'de> Deserialize<'de> for ArrangementEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(
            match name_or::<D, EntryBody>(
                deserializer,
                "a pattern's name, a `play` object, or a `layers` object",
            )? {
                NameOr::Name(name) => Self::Single(Layer::Name(name)),
                NameOr::Body(EntryBody::Played(play)) => Self::Single(Layer::Transformed(play)),
                NameOr::Body(EntryBody::Layered(layered)) => Self::Layered(layered),
            },
        )
    }
}

const ENTRY_KEYS: [&str; 4] = ["steps", "chord", "degree", "note"];

struct EntryVisitor;

impl<'de> Visitor<'de> for EntryVisitor {
    type Value = PatternEntry;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a pattern entry: an object with `note`, `degree`, `chord` or `steps`")
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<PatternEntry, A::Error> {
        let fields = Fields.visit_map(map)?;
        let Some(form) = ENTRY_KEYS
            .into_iter()
            .find(|key| fields.iter().any(|(name, _)| name == key))
        else {
            return Err(A::Error::custom(
                "a pattern entry names what it plays with one of `note`, `degree`, `chord` \
                 or `steps`, and this one has none of them",
            ));
        };
        let body = MapDeserializer::<_, serde_json::Error>::new(fields.into_iter());
        match form {
            "steps" => Steps::deserialize(body).map(PatternEntry::Steps),
            "chord" => Chord::deserialize(body).map(PatternEntry::Chord),
            "degree" => DegreeNote::deserialize(body).map(PatternEntry::Degree),
            _ => Note::deserialize(body).map(PatternEntry::Note),
        }
        .map_err(A::Error::custom)
    }
}

impl<'de> Deserialize<'de> for PatternEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(EntryVisitor)
    }
}
