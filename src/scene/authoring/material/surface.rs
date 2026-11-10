//! The transparent-surface fields of a material (#1523): URP Lit's blending and
//! shadow toggles and HDRP Lit's refraction, absorption, depth postpass, motion vectors,
//! SSR (opaque and transparent, #2004) and thin translucency, and TAA's reactivity (#1845), as rows of the material field table (`field.rs`).
//!
//! Pure.

use super::field::{no_hint, SurfaceField, SurfaceKind, SurfaceValue};
use crate::components::{BlendMode, MaterialAsset, RefractionModel, TaaReactive};

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
        hint: no_hint,
        put: |m, v| m.blend_mode = choice(v, BlendMode::parse),
    },
    SurfaceField {
        key: "preserve_specular",
        api: "PreserveSpecular",
        label: "Preserve Specular",
        kind: SurfaceKind::Toggle,
        shown: |m| m.is_transparent() && m.blend_mode.preserves_specular(),
        get: |m| SurfaceValue::Bool(m.preserve_specular),
        hint: no_hint,
        put: |m, v| m.preserve_specular = boolean(v),
    },
    SurfaceField {
        key: "receive_shadows",
        api: "ReceiveShadows",
        label: "Receive Shadows",
        kind: SurfaceKind::Toggle,
        shown: always,
        get: |m| SurfaceValue::Bool(m.receive_shadows),
        hint: no_hint,
        put: |m, v| m.receive_shadows = boolean(v),
    },
    SurfaceField {
        key: "refraction_model",
        api: "RefractionModel",
        label: "Refraction Model",
        kind: SurfaceKind::Choice(|| RefractionModel::ALL.map(RefractionModel::name).to_vec()),
        shown: transparent,
        get: |m| SurfaceValue::Text(m.refraction_model.name().to_owned()),
        hint: no_hint,
        put: |m, v| m.refraction_model = choice(v, RefractionModel::parse),
    },
    SurfaceField {
        key: "ior",
        api: "IndexOfRefraction",
        label: "Index of Refraction",
        kind: SurfaceKind::Number(1.0..=2.5),
        shown: refracts,
        get: |m| SurfaceValue::Number(m.ior),
        hint: no_hint,
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
        hint: no_hint,
        put: |m, v| m.thickness = number(v),
    },
    SurfaceField {
        key: "transmittance_color",
        api: "TransmittanceColor",
        label: "Transmittance Color",
        kind: SurfaceKind::Color,
        shown: transparent,
        get: |m| SurfaceValue::Color(m.transmittance_color),
        hint: no_hint,
        put: |m, v| m.transmittance_color = color(v),
    },
    SurfaceField {
        key: "absorption_distance",
        api: "AbsorptionDistance",
        label: "Absorption Distance",
        kind: SurfaceKind::Number(0.01..=100.0),
        shown: transparent,
        get: |m| SurfaceValue::Number(m.absorption_distance),
        hint: no_hint,
        put: |m, v| m.absorption_distance = number(v),
    },
    SurfaceField {
        key: "depth_postpass",
        api: "DepthPostpass",
        label: "Depth Postpass",
        kind: SurfaceKind::Toggle,
        shown: transparent,
        get: |m| SurfaceValue::Bool(m.depth_postpass),
        hint: no_hint,
        put: |m, v| m.depth_postpass = boolean(v),
    },
    SurfaceField {
        key: "writes_motion",
        api: "WritesMotion",
        label: "Writes Motion Vectors",
        kind: SurfaceKind::Toggle,
        shown: transparent,
        get: |m| SurfaceValue::Bool(m.writes_motion),
        hint: no_hint,
        put: |m, v| m.writes_motion = boolean(v),
    },
    SurfaceField {
        key: "reactive",
        api: "Reactive",
        label: "TAA Reactive",
        kind: SurfaceKind::Choice(|| TaaReactive::ALL.map(TaaReactive::name).to_vec()),
        shown: transparent,
        get: |m| SurfaceValue::Text(m.reactive.name().to_owned()),
        hint: no_hint,
        put: |m, v| m.reactive = choice(v, TaaReactive::parse),
    },
    SurfaceField {
        key: "receive_ssr",
        api: "ReceiveSsr",
        label: "Receive SSR",
        kind: SurfaceKind::Toggle,
        shown: always,
        get: |m| SurfaceValue::Bool(m.receives_ssr()),
        hint: no_hint,
        put: |m, v| m.receive_ssr = Some(boolean(v)),
    },
    SurfaceField {
        key: "transmission",
        api: "Transmission",
        label: "Transmission",
        kind: SurfaceKind::Number(0.0..=1.0),
        shown: always,
        get: |m| SurfaceValue::Number(m.transmission),
        hint: no_hint,
        put: |m, v| m.transmission = number(v),
    },
    SurfaceField {
        key: "transmission_color",
        api: "TransmissionColor",
        label: "Transmission Color",
        kind: SurfaceKind::Color,
        shown: |m| m.transmission > 0.0,
        get: |m| SurfaceValue::Color(m.transmission_color),
        hint: no_hint,
        put: |m, v| m.transmission_color = color(v),
    },
];

#[cfg(test)]
#[path = "surface_tests.rs"]
mod tests;
