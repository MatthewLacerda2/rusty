//! How a song's written forms are told apart when they are read — by hand,
//! rather than by `#[serde(untagged)]`, so a refusal still names what is wrong.
//!
//! Three places in a song accept more than one shape: a track's
//! [`PatchRef`] (a name, or the patch itself), an [`ArrangementEntry`] (a
//! name, a [`Play`] or a [`Layers`]) and a [`PatternEntry`] (a note, a
//! degree, a chord or a step string). They are still *written* untagged — the derive serialises
//! them, and the documents on disk do not change — but serde's untagged
//! **reader** tries each variant in turn and, when none fits, reports only
//! *"data did not match any variant"*. Every document type underneath denies
//! unknown fields, so a single misspelled key inside an inline patch or a note
//! lands there, and the one word an agent needs to fix it — the key — is
//! thrown away. That silence is what the strictness exists to end (#592).
//!
//! So each is read by deciding the variant **first**, from something that
//! cannot be misspelled, and then handing the whole value to that one
//! variant's own reader, whose error is passed through as it stands:
//!
//! - a name or a document is decided by the **JSON type** — a string is a
//!   name, an object is the long form — which is the same rule the untagged
//!   derive relied on, now stated rather than discovered;
//! - an arrangement entry written as an object is decided by **whether it
//!   carries `layers`**: with it, the whole object is a [`Layers`], so a
//!   `pattern` beside it is refused by name rather than one of the two being
//!   quietly dropped; without it, a [`Play`];
//! - a pattern entry is decided by **which of its four keys is present**,
//!   looked for in the order `steps`, `chord`, `degree`, `note`. `steps` comes
//!   first because a [`Steps`] may also carry a `note`, and `note` last for
//!   the same reason. An entry carrying two of them goes to the first and is
//!   refused by it for the second, *by name* — still refused rather than
//!   resolved, as it always was, and now saying why.

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

/// A value written either as a bare name or as the document it names.
enum NameOr<T> {
    /// A JSON string.
    Name(String),
    /// A JSON object, read as `T` — so `T`'s own refusal is the one reported.
    Body(T),
}

/// Reads a [`NameOr`], saying `what` in the one refusal that is its own: a
/// value that is neither a string nor an object.
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

/// Reads a name-or-document value from `deserializer`.
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

/// Reads an object's keys and values as written — every key, duplicates
/// included — so a form chosen afterwards reads exactly what the page says.
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

/// The long form of an [`ArrangementEntry`], before it is known which one.
enum EntryBody {
    /// It carried `layers`.
    Layered(Layers),
    /// It did not.
    Played(Play),
}

impl<'de> Deserialize<'de> for EntryBody {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Buffered for the reason a pattern entry is: the form is decided by a
        // key, and the form's own reader then sees exactly what was written.
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

/// The key that decides each form of [`PatternEntry`], in the order they are
/// looked for — see the module doc for why this order.
const ENTRY_KEYS: [&str; 4] = ["steps", "chord", "degree", "note"];

/// Reads a [`PatternEntry`] by the key it carries.
struct EntryVisitor;

impl<'de> Visitor<'de> for EntryVisitor {
    type Value = PatternEntry;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a pattern entry: an object with `note`, `degree`, `chord` or `steps`")
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<PatternEntry, A::Error> {
        // Buffered as written, so a key written twice is refused as a
        // duplicate rather than quietly collapsed.
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
