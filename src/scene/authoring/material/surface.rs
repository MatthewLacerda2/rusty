//! The transparent-surface fields of a material (#1523): URP Lit's blending and
//! shadow toggles and HDRP Lit's refraction, absorption, depth postpass and thin
//! translucency, as one table.
//!
//! Each [`SurfaceField`] row is the field's one write and its validation, its name
//! in a recipe and a scene file (`key`), and its `Material.Set<api>` /
//! `Material.Get<api>` pair. The Lua adapter, `Material.DefineAsset`'s recipe and
//! the Inspector's Material card all walk [`SURFACE_FIELDS`], so a new field adds
//! one row here and nothing to those three.
//!
//! Pure.

use std::ops::RangeInclusive;

use super::super::sanitize::{finite_array_or, saturated, unit};
use super::MaterialLibrary;
use crate::components::{BlendMode, MaterialAsset};

/// A surface field's value, as the API, a recipe and the Inspector carry it.
#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceValue {
    /// A choice, by name (`"Additive"`), read in any case.
    Text(String),
    Bool(bool),
    Number(f32),
    Color([f32; 3]),
}

/// What kind of value a field holds, and the widget the Inspector draws for it.
#[derive(Clone, Debug)]
pub enum SurfaceKind {
    /// One of these names (an enum's `ALL`, named).
    Choice(fn() -> Vec<&'static str>),
    Toggle,
    /// A number the write clamps into this range.
    Number(RangeInclusive<f32>),
    /// An RGB colour, each channel clamped to `[0, 1]`.
    Color,
}

/// One transparent-surface field of [`MaterialAsset`].
pub struct SurfaceField {
    /// Its name in a scene file and a `DefineAsset` recipe (`"blend_mode"`).
    pub key: &'static str,
    /// Its API name: `Material.Set<api>` / `Material.Get<api>`.
    pub api: &'static str,
    /// Its Inspector label.
    pub label: &'static str,
    pub kind: SurfaceKind,
    /// Shown in the Inspector only while this holds (a knob that would be inert is
    /// hidden, as `alpha` is on an opaque material).
    pub shown: fn(&MaterialAsset) -> bool,
    pub get: fn(&MaterialAsset) -> SurfaceValue,
    /// The write, after [`SurfaceField::check`] made `value` the right kind.
    put: fn(&mut MaterialAsset, SurfaceValue),
}

impl SurfaceField {
    /// `value` as this field takes it: the kind matches, a number or colour clamped,
    /// a choice parsed. An unknown choice or a value of another kind is an error
    /// naming what the field takes.
    pub fn check(&self, value: SurfaceValue) -> Result<SurfaceValue, String> {
        let wrong = || self.wrong_kind();
        match (&self.kind, value) {
            (SurfaceKind::Choice(names), SurfaceValue::Text(t)) => names()
                .into_iter()
                .find(|n| n.eq_ignore_ascii_case(&t))
                .map(|n| SurfaceValue::Text(n.to_owned()))
                .ok_or_else(wrong),
            (SurfaceKind::Toggle, SurfaceValue::Bool(b)) => Ok(SurfaceValue::Bool(b)),
            (SurfaceKind::Number(r), SurfaceValue::Number(n)) => {
                Ok(SurfaceValue::Number(saturated(n, *r.start(), *r.end())))
            }
            (SurfaceKind::Color, SurfaceValue::Color(c)) => {
                Ok(SurfaceValue::Color(finite_array_or(c, [1.0; 3]).map(unit)))
            }
            _ => Err(wrong()),
        }
    }

    /// The error for a value this field cannot take, naming what it takes.
    pub fn wrong_kind(&self) -> String {
        format!("{} takes {}", self.key, self.kind.describe())
    }
}

impl SurfaceKind {
    /// What a value of this kind is, for an error text.
    fn describe(&self) -> String {
        match self {
            Self::Choice(names) => format!("one of {}", names().join(", ")),
            Self::Toggle => "a boolean".into(),
            Self::Number(r) => format!("a number in [{}, {}]", r.start(), r.end()),
            Self::Color => "an {r, g, b} colour".into(),
        }
    }
}

/// The row whose `key` or `api` is `name` (in any case).
pub fn surface_field(name: &str) -> Option<&'static SurfaceField> {
    SURFACE_FIELDS
        .iter()
        .find(|f| f.key.eq_ignore_ascii_case(name) || f.api.eq_ignore_ascii_case(name))
}

/// Write `value` into material `key`'s `field`, validated by the row. A missing
/// material changes nothing.
pub fn set_surface(
    materials: &mut MaterialLibrary,
    key: &str,
    field: &SurfaceField,
    value: SurfaceValue,
) -> Result<(), String> {
    let value = field.check(value)?;
    if let Some(m) = materials.get_mut(key) {
        (field.put)(m, value);
    }
    Ok(())
}

fn transparent(m: &MaterialAsset) -> bool {
    m.is_transparent()
}

fn always(_: &MaterialAsset) -> bool {
    true
}

/// A choice the row's `check` parsed, back to its variant.
fn choice<T: Default>(value: SurfaceValue, parse: fn(&str) -> Option<T>) -> T {
    match value {
        SurfaceValue::Text(t) => parse(&t).unwrap_or_default(),
        _ => T::default(),
    }
}

fn boolean(value: SurfaceValue) -> bool {
    matches!(value, SurfaceValue::Bool(true))
}

/// Every transparent-surface field, in Inspector order.
pub static SURFACE_FIELDS: &[SurfaceField] = &[
    SurfaceField {
        key: "blend_mode",
        api: "BlendMode",
        label: "Blending Mode",
        kind: SurfaceKind::Choice(|| BlendMode::ALL.map(BlendMode::name).to_vec()),
        shown: transparent,
        get: |m| SurfaceValue::Text(m.blend_mode.name().to_owned()),
        put: |m, v| m.blend_mode = choice(v, BlendMode::parse),
    },
    SurfaceField {
        key: "preserve_specular",
        api: "PreserveSpecular",
        label: "Preserve Specular",
        kind: SurfaceKind::Toggle,
        shown: |m| m.is_transparent() && m.blend_mode.preserves_specular(),
        get: |m| SurfaceValue::Bool(m.preserve_specular),
        put: |m, v| m.preserve_specular = boolean(v),
    },
    SurfaceField {
        key: "receive_shadows",
        api: "ReceiveShadows",
        label: "Receive Shadows",
        kind: SurfaceKind::Toggle,
        shown: always,
        get: |m| SurfaceValue::Bool(m.receive_shadows),
        put: |m, v| m.receive_shadows = boolean(v),
    },
];

#[cfg(test)]
#[path = "surface_tests.rs"]
mod tests;
