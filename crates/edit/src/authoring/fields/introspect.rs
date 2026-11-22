use serde::de::{self, Deserialize, Visitor};

/// Records the list a derived `Deserialize` hands `deserialize_struct`, then stops;
/// `#[serde(skip)]` fields are not in it.
#[cfg(test)]
pub fn serde_fields<'de, T: Deserialize<'de>>() -> Option<&'static [&'static str]> {
    let mut found = None;
    let _ = T::deserialize(Probe(&mut found));
    found
}

#[cfg(test)]
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

#[cfg(test)]
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

/// Whether `name` is optional, and the Lua type it reads as: `None` where the derive does not say.
pub fn field_type<'de, T: Deserialize<'de>>(name: &'static str) -> (bool, Option<&'static str>) {
    let mut found = Shape::default();
    let _ = T::deserialize(FieldProbe {
        name,
        out: &mut found,
    });
    (found.optional, found.lua)
}

#[derive(Default)]
struct Shape {
    optional: bool,
    lua: Option<&'static str>,
}

struct FieldProbe<'a> {
    name: &'static str,
    out: &'a mut Shape,
}

impl<'de> de::Deserializer<'de> for FieldProbe<'_> {
    type Error = Stop;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Stop> {
        visitor.visit_map(OneField {
            name: Some(self.name),
            out: self.out,
        })
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

struct OneField<'a> {
    name: Option<&'static str>,
    out: &'a mut Shape,
}

impl<'de> de::MapAccess<'de> for OneField<'_> {
    type Error = Stop;

    fn next_key_seed<K: de::DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Stop> {
        match self.name.take() {
            Some(name) => seed
                .deserialize(de::value::BorrowedStrDeserializer::new(name))
                .map(Some),
            None => Err(Stop),
        }
    }

    fn next_value_seed<V: de::DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Stop> {
        seed.deserialize(TypeProbe(self.out))
    }
}

struct TypeProbe<'a>(&'a mut Shape);

impl TypeProbe<'_> {
    fn found<T>(self, lua: &'static str) -> Result<T, Stop> {
        self.0.lua = Some(lua);
        Err(Stop)
    }
}

macro_rules! probe_as {
    ($lua:literal: $($method:ident)*) => {$(
        fn $method<V: Visitor<'de>>(self, _: V) -> Result<V::Value, Stop> {
            self.found($lua)
        }
    )*};
}

impl<'de> de::Deserializer<'de> for TypeProbe<'_> {
    type Error = Stop;

    fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, Stop> {
        Err(Stop)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Stop> {
        self.0.optional = true;
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        visitor: V,
    ) -> Result<V::Value, Stop> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: V,
    ) -> Result<V::Value, Stop> {
        self.found("nil")
    }

    fn deserialize_tuple<V: Visitor<'de>>(self, _: usize, _: V) -> Result<V::Value, Stop> {
        self.found("table")
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: usize,
        _: V,
    ) -> Result<V::Value, Stop> {
        self.found("table")
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        _: V,
    ) -> Result<V::Value, Stop> {
        self.found("table")
    }

    // For an enum carrying data, `Component.Fields` takes "table" from the default value
    // (when there is one), which wins.
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        _: V,
    ) -> Result<V::Value, Stop> {
        self.found("string")
    }

    probe_as!("boolean": deserialize_bool);
    probe_as!("number": deserialize_i8 deserialize_i16 deserialize_i32 deserialize_i64
        deserialize_i128 deserialize_u8 deserialize_u16 deserialize_u32 deserialize_u64
        deserialize_u128 deserialize_f32 deserialize_f64);
    probe_as!("string": deserialize_char deserialize_str deserialize_string deserialize_bytes
        deserialize_byte_buf deserialize_identifier);
    probe_as!("table": deserialize_seq deserialize_map);
    probe_as!("nil": deserialize_unit);

    fn deserialize_ignored_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, Stop> {
        Err(Stop)
    }
}
