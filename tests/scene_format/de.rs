//! The tracer as a serde `Deserializer`: each request records the kind it asks for
//! at the current place, then answers with a sample so the walk goes on.

use serde::de::value::SeqDeserializer;
use serde::de::{Deserializer, Visitor};
use serde::forward_to_deserialize_any;

use super::access::{Fields, One, Variant};
use super::trace::{Error, Pass};

/// `Display` for a visitor's `expecting`, which names the type serde wants.
struct Expecting<'v, V>(&'v V);

impl<'de, V: Visitor<'de>> std::fmt::Display for Expecting<'_, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.expecting(f)
    }
}

macro_rules! leaf {
    ($($method:ident => $kind:literal $visit:ident($($sample:expr)?);)*) => {$(
        fn $method<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            self.line($kind);
            v.$visit($($sample)?)
        }
    )*};
}

impl<'de> Deserializer<'de> for &mut Pass<'_> {
    type Error = Error;

    leaf! {
        deserialize_bool => "bool" visit_bool(true);
        deserialize_i8 => "i8" visit_i8(1);
        deserialize_i16 => "i16" visit_i16(1);
        deserialize_i32 => "i32" visit_i32(1);
        deserialize_i64 => "i64" visit_i64(1);
        deserialize_u8 => "u8" visit_u8(1);
        deserialize_u16 => "u16" visit_u16(1);
        deserialize_u32 => "u32" visit_u32(1);
        deserialize_u64 => "u64" visit_u64(1);
        deserialize_f32 => "f32" visit_f32(0.5);
        deserialize_f64 => "f64" visit_f64(0.5);
        deserialize_char => "char" visit_char('c');
        deserialize_str => "string" visit_str("s");
        deserialize_string => "string" visit_str("s");
        deserialize_bytes => "bytes" visit_bytes(b"s");
        deserialize_byte_buf => "bytes" visit_bytes(b"s");
        deserialize_unit => "unit" visit_unit();
    }

    /// Opaque to serde: answer by the place's current guess (see `ANY_ANSWERS`).
    fn deserialize_any<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        self.line("any");
        if let Some(sample) = super::hidden::opaque(&self.path) {
            return serde::Deserializer::deserialize_any(sample, v)
                .map_err(serde::de::Error::custom);
        }
        self.last_any = Some(self.path.clone());
        match self.any.get(&self.path).copied().unwrap_or(0) {
            0 => v.visit_f64(0.5),
            1 => v.visit_seq(SeqDeserializer::new([0.5f64; 4].into_iter())),
            _ => v.visit_unit(),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        self.line("option");
        v.visit_some(self)
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(self, _: &str, v: V) -> Result<V::Value, Error> {
        self.line("unit");
        v.visit_unit()
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(self, _: &str, v: V) -> Result<V::Value, Error> {
        v.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        self.line("seq");
        v.visit_seq(One::seq(self))
    }

    fn deserialize_tuple<V: Visitor<'de>>(self, len: usize, v: V) -> Result<V::Value, Error> {
        self.line(&format!("tuple({len})"));
        v.visit_seq(One::tuple(self, len))
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _: &str,
        len: usize,
        v: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_tuple(len, v)
    }

    /// A map, or a struct with a `#[serde(flatten)]` field: serde asks for both the
    /// same way and only `expecting` tells them apart.
    fn deserialize_map<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        let expecting = Expecting(&v).to_string();
        if let Some(name) = expecting.strip_prefix("struct ") {
            return super::hidden::visit(self, name, v);
        }
        self.line("map");
        v.visit_map(One::map(self))
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _: &str,
        fields: &'static [&'static str],
        v: V,
    ) -> Result<V::Value, Error> {
        if fields.is_empty() {
            self.line("{}");
        }
        v.visit_map(Fields::new(self, fields))
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _: &str,
        variants: &'static [&'static str],
        v: V,
    ) -> Result<V::Value, Error> {
        self.line(&format!("enum {}", variants.join("|")));
        self.enums.push((self.path.clone(), variants.len()));
        let chosen = self.script.get(&self.path).copied().unwrap_or(0);
        v.visit_enum(Variant::new(self, variants[chosen]))
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        v.visit_unit()
    }

    forward_to_deserialize_any! { i128 u128 identifier }
}
