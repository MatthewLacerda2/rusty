//! The tracer's collection, struct and enum accessors: each hands serde one sample
//! element per place, recorded one segment deeper.

use serde::de::{
    DeserializeSeed, EnumAccess, IntoDeserializer, MapAccess, SeqAccess, VariantAccess, Visitor,
};

use super::trace::{Error, Pass};

/// A sequence, tuple or map: `left` samples, elements at `[]`, map keys at `{key}`
/// and values at `{}`.
pub struct One<'p, 's> {
    pass: &'p mut Pass<'s>,
    left: usize,
}

impl<'p, 's> One<'p, 's> {
    pub fn seq(pass: &'p mut Pass<'s>) -> Self {
        Self { pass, left: 1 }
    }
    pub fn tuple(pass: &'p mut Pass<'s>, len: usize) -> Self {
        Self { pass, left: len }
    }
    pub fn map(pass: &'p mut Pass<'s>) -> Self {
        Self { pass, left: 1 }
    }
}

impl<'de> SeqAccess<'de> for One<'_, '_> {
    type Error = Error;
    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        if self.left == 0 {
            return Ok(None);
        }
        self.left -= 1;
        self.pass.at("[]", |p| seed.deserialize(p)).map(Some)
    }
}

impl<'de> MapAccess<'de> for One<'_, '_> {
    type Error = Error;
    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        if self.left == 0 {
            return Ok(None);
        }
        self.left -= 1;
        self.pass.at("{key}", |p| seed.deserialize(p)).map(Some)
    }
    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
        self.pass.at("{}", |p| seed.deserialize(p))
    }
}

/// A struct: every field serde names, each at `.field`.
pub struct Fields<'p, 's> {
    pass: &'p mut Pass<'s>,
    fields: std::slice::Iter<'static, &'static str>,
    current: &'static str,
}

impl<'p, 's> Fields<'p, 's> {
    pub fn new(pass: &'p mut Pass<'s>, fields: &'static [&'static str]) -> Self {
        Self {
            pass,
            fields: fields.iter(),
            current: "",
        }
    }
}

impl<'de> MapAccess<'de> for Fields<'_, '_> {
    type Error = Error;
    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        let field = loop {
            let Some(&field) = self.fields.next() else {
                return Ok(None);
            };
            let place = format!("{}.{field}", self.pass.path);
            if !self.pass.aliases.contains(&place) {
                self.pass.last_field = Some(place);
                break field;
            }
            self.pass.lines.insert(format!("{place}: alias"));
        };
        self.current = field;
        seed.deserialize(field.into_deserializer()).map(Some)
    }
    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
        let segment = format!(".{}", self.current);
        self.pass.at(&segment, |p| seed.deserialize(p))
    }
}

/// The variant a pass picked, its contents at `<Variant>`.
pub struct Variant<'p, 's> {
    pass: &'p mut Pass<'s>,
    name: &'static str,
}

impl<'p, 's> Variant<'p, 's> {
    pub fn new(pass: &'p mut Pass<'s>, name: &'static str) -> Self {
        Self { pass, name }
    }
}

impl<'de, 'p, 's> EnumAccess<'de> for Variant<'p, 's> {
    type Error = Error;
    type Variant = Self;
    fn variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<(T::Value, Self), Error> {
        let tag = seed.deserialize(self.name.into_deserializer())?;
        Ok((tag, self))
    }
}

impl<'de> VariantAccess<'de> for Variant<'_, '_> {
    type Error = Error;
    fn unit_variant(self) -> Result<(), Error> {
        Ok(())
    }
    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
        let segment = format!("<{}>", self.name);
        self.pass.at(&segment, |p| seed.deserialize(p))
    }
    fn tuple_variant<V: Visitor<'de>>(self, len: usize, v: V) -> Result<V::Value, Error> {
        let segment = format!("<{}>", self.name);
        self.pass.at(&segment, |p| {
            serde::Deserializer::deserialize_tuple(p, len, v)
        })
    }
    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        v: V,
    ) -> Result<V::Value, Error> {
        let segment = format!("<{}>", self.name);
        self.pass.at(&segment, |p| {
            serde::Deserializer::deserialize_struct(p, "", fields, v)
        })
    }
}
