//! src/api/component/introspect.rs — a component's serialized field names, read
//! from its serde derive (test-only).
//!
//! A derived `Deserialize` hands its struct's field list to
//! `Deserializer::deserialize_struct`; [`serde_fields`] is a deserializer that
//! records that list and stops. It is how the field-table test learns every field
//! a component saves, with no instance to build and no list to keep by hand.
//! `#[serde(skip)]` fields are not in the list, which is the point: runtime state
//! is never exposed.

use serde::de::{self, Deserialize, Visitor};

/// The field names `T`'s derived `Deserialize` declares, or `None` when `T` is not
/// read as a struct (a hand-written impl, a map, a newtype).
pub fn serde_fields<'de, T: Deserialize<'de>>() -> Option<&'static [&'static str]> {
    let mut found = None;
    let _ = T::deserialize(Probe(&mut found));
    found
}

struct Probe<'a>(&'a mut Option<&'static [&'static str]>);

#[derive(Debug)]
struct Stop;

impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("introspection probe")
    }
}

impl std::error::Error for Stop {}

impl de::Error for Stop {
    fn custom<M: std::fmt::Display>(_: M) -> Self {
        Stop
    }
}

impl<'de> de::Deserializer<'de> for Probe<'_> {
    type Error = Stop;

    fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, Stop> {
        Err(Stop)
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        fields: &'static [&'static str],
        _: V,
    ) -> Result<V::Value, Stop> {
        *self.0 = Some(fields);
        Err(Stop)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map enum identifier ignored_any
    }
}
