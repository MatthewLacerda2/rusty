//! The material field table's types (#1523): a [`SurfaceField`] row is the
//! field's one write and its validation, its name in a recipe and a scene file
//! (`key`), and its `Material.Set<api>` / `Material.Get<api>` pair. The Lua adapter,
//! `Material.DefineAsset`'s recipe and the Inspector's Material card all walk
//! [`material_fields`] (the render face's row in `face.rs`, the transparent
//! surface's in `surface.rs`, the map coordinates' in `coordinates.rs`, then the
//! detail layer's in `detail.rs`), so a
//! new field adds one row and nothing to those three.
//!
//! Pure.

use std::ops::RangeInclusive;

use super::super::sanitize::{finite_array_or, saturated};
use super::MaterialLibrary;
use crate::components::MaterialAsset;

/// A surface field's value, as the API, a recipe and the Inspector carry it.
#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceValue {
    /// A choice, by name (`"Additive"`), read in any case.
    Text(String),
    Bool(bool),
    Number(f32),
    Color([f32; 3]),
    Vec2([f32; 2]),
}

/// What kind of value a field holds, and the widget the Inspector draws for it.
#[derive(Clone, Debug)]
pub enum SurfaceKind {
    /// One of these names (an enum's `ALL`, named).
    Choice(fn() -> Vec<&'static str>),
    Toggle,
    /// A number the write clamps into this range.
    Number(RangeInclusive<f32>),
    /// An RGB colour, each channel clamped into this range: `[0, 1]` for a colour,
    /// past 1 for a tint that can also raise (`specular_color`, #1998).
    Color(RangeInclusive<f32>),
    /// A pair of numbers, each clamped into this range (tiling, offset).
    Vec2(RangeInclusive<f32>),
}

/// One row of a material's field table: its names, its value kind and its write.
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
    /// The Inspector's hover text for the row on this material, if it has one: a
    /// meaning that depends on the material (Offset's sign under each mapping, #1851).
    pub hint: fn(&MaterialAsset) -> Option<&'static str>,
    /// The write, after [`SurfaceField::check`] made `value` the right kind.
    pub(super) put: fn(&mut MaterialAsset, SurfaceValue),
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
            (SurfaceKind::Color(r), SurfaceValue::Color(c)) => Ok(SurfaceValue::Color(
                finite_array_or(c, [1.0; 3]).map(|n| saturated(n, *r.start(), *r.end())),
            )),
            (SurfaceKind::Vec2(r), SurfaceValue::Vec2(v)) => Ok(SurfaceValue::Vec2(
                v.map(|n| saturated(n, *r.start(), *r.end())),
            )),
            _ => Err(wrong()),
        }
    }

    /// The error for a value this field cannot take, naming what it takes.
    pub fn wrong_kind(&self) -> String {
        format!("{} takes {}", self.key, self.kind.describe())
    }
}

/// A row's `hint` when it has none.
pub(super) fn no_hint(_: &MaterialAsset) -> Option<&'static str> {
    None
}

impl SurfaceKind {
    /// What a value of this kind is, for an error text.
    fn describe(&self) -> String {
        match self {
            Self::Choice(names) => format!("one of {}", names().join(", ")),
            Self::Toggle => "a boolean".into(),
            Self::Number(r) => format!("a number in [{}, {}]", r.start(), r.end()),
            Self::Color(r) => format!(
                "an {{r, g, b}} colour, each in [{}, {}]",
                r.start(),
                r.end()
            ),
            Self::Vec2(r) => format!("an {{x, y}} pair, each in [{}, {}]", r.start(), r.end()),
        }
    }
}

/// Every material field the table drives: the render face (#1615), the
/// transparent-surface rows, the map coordinates' (#1573), then the detail
/// layer's (#1997).
pub fn material_fields() -> impl Iterator<Item = &'static SurfaceField> {
    let rows = super::FACE_FIELDS.iter().chain(super::SURFACE_FIELDS);
    rows.chain(super::COORDINATE_FIELDS)
        .chain(super::DETAIL_FIELDS)
}

/// The row whose `key` or `api` is `name` (in any case).
pub fn surface_field(name: &str) -> Option<&'static SurfaceField> {
    material_fields().find(|f| f.key.eq_ignore_ascii_case(name) || f.api.eq_ignore_ascii_case(name))
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
