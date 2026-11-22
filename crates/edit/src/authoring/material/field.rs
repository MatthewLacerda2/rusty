use std::ops::RangeInclusive;

use super::MaterialLibrary;
use rusty_components::MaterialAsset;
use rusty_scene::rules::sanitize::{finite_array_or, saturated};

#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceValue {
    Text(String),
    Bool(bool),
    Number(f32),
    Color([f32; 3]),
    Vec2([f32; 2]),
}

#[derive(Clone, Debug)]
pub enum SurfaceKind {
    Choice(fn() -> Vec<&'static str>),
    Toggle,
    Number(RangeInclusive<f32>),
    /// `[0, 1]` for a colour, past 1 for a tint that can also raise.
    Color(RangeInclusive<f32>),
    Vec2(RangeInclusive<f32>),
}

pub struct SurfaceField {
    /// Its name in a `DefineAsset` recipe (`"blend_mode"`); `api` names its `Material.Set<api>`.
    pub key: &'static str,
    pub api: &'static str,
    pub label: &'static str,
    pub kind: SurfaceKind,
    pub shown: fn(&MaterialAsset) -> bool,
    pub get: fn(&MaterialAsset) -> SurfaceValue,
    pub hint: fn(&MaterialAsset) -> Option<&'static str>,
    /// Takes only what [`SurfaceField::check`] returned: the right kind, clamped or parsed.
    pub(super) put: fn(&mut MaterialAsset, SurfaceValue),
}

impl SurfaceField {
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

    pub fn wrong_kind(&self) -> String {
        format!("{} takes {}", self.key, self.kind.describe())
    }
}

pub(super) fn no_hint(_: &MaterialAsset) -> Option<&'static str> {
    None
}

impl SurfaceKind {
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

/// A new table joins here and the Inspector's Material card
/// (`editor::inspector::components::material`), which draws each table itself.
pub fn material_fields() -> impl Iterator<Item = &'static SurfaceField> {
    let rows = super::FACE_FIELDS.iter().chain(super::SURFACE_FIELDS);
    rows.chain(super::COORDINATE_FIELDS)
        .chain(super::DETAIL_FIELDS)
}

pub fn surface_field(name: &str) -> Option<&'static SurfaceField> {
    material_fields().find(|f| f.key.eq_ignore_ascii_case(name) || f.api.eq_ignore_ascii_case(name))
}

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
