//! The transparent-surface fields of a material (#1523): URP Lit's blending and
//! shadow toggles and HDRP Lit's refraction, absorption, depth postpass, transparent
//! SSR and thin translucency, as one table.
//!
//! Each [`SurfaceField`] row is the field's one write and its validation, its name
//! in a recipe and a scene file (`key`), and its `Material.Set<api>` /
//! `Material.Get<api>` pair. The Lua adapter, `Material.DefineAsset`'s recipe and
//! the Inspector's Material card all walk [`material_fields`] (these rows, then the
//! map coordinates' in `coordinates.rs`), so a new field adds one row and nothing to
//! those three.
//!
//! Pure.

use std::ops::RangeInclusive;

use super::super::sanitize::{finite_array_or, saturated, unit};
use super::MaterialLibrary;
use crate::components::{BlendMode, MaterialAsset, RefractionModel};

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
    /// An RGB colour, each channel clamped to `[0, 1]`.
    Color,
    /// A pair of numbers, each clamped into this range (tiling, offset).
    Vec2(RangeInclusive<f32>),
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
            (SurfaceKind::Color, SurfaceValue::Color(c)) => {
                Ok(SurfaceValue::Color(finite_array_or(c, [1.0; 3]).map(unit)))
            }
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

impl SurfaceKind {
    /// What a value of this kind is, for an error text.
    fn describe(&self) -> String {
        match self {
            Self::Choice(names) => format!("one of {}", names().join(", ")),
            Self::Toggle => "a boolean".into(),
            Self::Number(r) => format!("a number in [{}, {}]", r.start(), r.end()),
            Self::Color => "an {r, g, b} colour".into(),
            Self::Vec2(r) => format!("an {{x, y}} pair, each in [{}, {}]", r.start(), r.end()),
        }
    }
}

/// Every material field the table drives: the render face (#1615), the
/// transparent-surface rows, then the map coordinates' (#1573).
pub fn material_fields() -> impl Iterator<Item = &'static SurfaceField> {
    let rows = super::FACE_FIELDS.iter().chain(SURFACE_FIELDS);
    rows.chain(super::COORDINATE_FIELDS)
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

fn transparent(m: &MaterialAsset) -> bool {
    m.is_transparent()
}

pub(super) fn always(_: &MaterialAsset) -> bool {
    true
}

/// A choice the row's `check` parsed, back to its variant.
pub(super) fn choice<T: Default>(value: SurfaceValue, parse: fn(&str) -> Option<T>) -> T {
    match value {
        SurfaceValue::Text(t) => parse(&t).unwrap_or_default(),
        _ => T::default(),
    }
}

fn boolean(value: SurfaceValue) -> bool {
    matches!(value, SurfaceValue::Bool(true))
}

/// A number the row's `check` clamped (0 for a value of another kind, which
/// `check` never passes).
pub(super) fn number(value: SurfaceValue) -> f32 {
    match value {
        SurfaceValue::Number(n) => n,
        _ => 0.0,
    }
}

/// A colour the row's `check` clamped.
fn color(value: SurfaceValue) -> [f32; 3] {
    match value {
        SurfaceValue::Color(c) => c,
        _ => [1.0; 3],
    }
}

fn refracts(m: &MaterialAsset) -> bool {
    m.is_transparent() && m.refraction_model != RefractionModel::None
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
    SurfaceField {
        key: "refraction_model",
        api: "RefractionModel",
        label: "Refraction Model",
        kind: SurfaceKind::Choice(|| RefractionModel::ALL.map(RefractionModel::name).to_vec()),
        shown: transparent,
        get: |m| SurfaceValue::Text(m.refraction_model.name().to_owned()),
        put: |m, v| m.refraction_model = choice(v, RefractionModel::parse),
    },
    SurfaceField {
        key: "ior",
        api: "IndexOfRefraction",
        label: "Index of Refraction",
        kind: SurfaceKind::Number(1.0..=2.5),
        shown: refracts,
        get: |m| SurfaceValue::Number(m.ior),
        put: |m, v| m.ior = number(v),
    },
    SurfaceField {
        key: "thickness",
        api: "Thickness",
        label: "Thickness",
        kind: SurfaceKind::Number(0.0..=10.0),
        // A Planar slab's depth, and the depth an absorbing surface absorbs over
        // against the sky.
        shown: |m| {
            m.is_transparent()
                && (m.refraction_model == RefractionModel::Planar
                    || m.transmittance_color != [1.0; 3])
        },
        get: |m| SurfaceValue::Number(m.thickness),
        put: |m, v| m.thickness = number(v),
    },
    SurfaceField {
        key: "transmittance_color",
        api: "TransmittanceColor",
        label: "Transmittance Color",
        kind: SurfaceKind::Color,
        shown: transparent,
        get: |m| SurfaceValue::Color(m.transmittance_color),
        put: |m, v| m.transmittance_color = color(v),
    },
    SurfaceField {
        key: "absorption_distance",
        api: "AbsorptionDistance",
        label: "Absorption Distance",
        kind: SurfaceKind::Number(0.01..=100.0),
        shown: transparent,
        get: |m| SurfaceValue::Number(m.absorption_distance),
        put: |m, v| m.absorption_distance = number(v),
    },
    SurfaceField {
        key: "depth_postpass",
        api: "DepthPostpass",
        label: "Depth Postpass",
        kind: SurfaceKind::Toggle,
        shown: transparent,
        get: |m| SurfaceValue::Bool(m.depth_postpass),
        put: |m, v| m.depth_postpass = boolean(v),
    },
    SurfaceField {
        key: "receive_ssr",
        api: "ReceiveSsr",
        label: "Receive SSR",
        kind: SurfaceKind::Toggle,
        shown: transparent,
        get: |m| SurfaceValue::Bool(m.receive_ssr),
        put: |m, v| m.receive_ssr = boolean(v),
    },
    SurfaceField {
        key: "transmission",
        api: "Transmission",
        label: "Transmission",
        kind: SurfaceKind::Number(0.0..=1.0),
        shown: always,
        get: |m| SurfaceValue::Number(m.transmission),
        put: |m, v| m.transmission = number(v),
    },
    SurfaceField {
        key: "transmission_color",
        api: "TransmissionColor",
        label: "Transmission Color",
        kind: SurfaceKind::Color,
        shown: |m| m.transmission > 0.0,
        get: |m| SurfaceValue::Color(m.transmission_color),
        put: |m, v| m.transmission_color = color(v),
    },
];

#[cfg(test)]
#[path = "surface_tests.rs"]
mod tests;
