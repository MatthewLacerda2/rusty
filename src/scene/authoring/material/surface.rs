//! The surface fields of a material (#1523): URP Lit's blending and shadow toggles,
//! glTF's `KHR_materials_ior` and `KHR_materials_specular` reflectance (#1998), and
//! HDRP Lit's refraction, absorption, sorting priority (#2039), depth postpass, motion vectors,
//! SSR (opaque and transparent, #2004) and thin translucency, and TAA's reactivity (#1845), as rows of the material field table (`field.rs`).
//!
//! Pure.

use super::field::{no_hint, SurfaceField, SurfaceKind, SurfaceValue};
use crate::components::{
    BlendMode, MaterialAsset, RefractionModel, TaaReactive, SORTING_PRIORITY_RANGE,
};

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

/// The channel range of a colour that can only tint (`transmittance_color`,
/// `transmission_color`); `specular_color` can also raise, up to 25.
const UNIT: std::ops::RangeInclusive<f32> = 0.0..=1.0;

/// HDRP's *Sorting Priority* slider (#2039), an integer the write rounds to.
const PRIORITY: std::ops::RangeInclusive<f32> =
    -(SORTING_PRIORITY_RANGE as f32)..=SORTING_PRIORITY_RANGE as f32;

/// Every surface field (blending, reflectance, refraction, absorption, SSR,
/// translucency, reactivity), in Inspector order.
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
        // It bends a refracting surface and sets every dielectric's reflectance
        // (#1998, `MaterialAsset::dielectric_f0`).
        shown: always,
        get: |m| SurfaceValue::Number(m.ior),
        hint: no_hint,
        put: |m, v| m.ior = number(v),
    },
    SurfaceField {
        key: "specular",
        api: "Specular",
        label: "Specular",
        kind: SurfaceKind::Number(0.0..=1.0),
        shown: always,
        get: |m| SurfaceValue::Number(m.specular),
        hint: no_hint,
        put: |m, v| m.specular = number(v),
    },
    SurfaceField {
        key: "specular_color",
        api: "SpecularColor",
        label: "Specular Color",
        // Above 1 raises a dielectric's reflectance, as glTF's extension allows; 25
        // takes glass (0.04) to a mirror.
        kind: SurfaceKind::Color(0.0..=25.0),
        shown: always,
        get: |m| SurfaceValue::Color(m.specular_color),
        hint: no_hint,
        put: |m, v| m.specular_color = color(v),
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
        kind: SurfaceKind::Color(UNIT),
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
        key: "sorting_priority",
        api: "SortingPriority",
        label: "Sorting Priority",
        kind: SurfaceKind::Number(PRIORITY),
        shown: transparent,
        get: |m| SurfaceValue::Number(m.sorting_priority as f32),
        hint: no_hint,
        put: |m, v| m.sorting_priority = number(v).round() as i32,
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
        kind: SurfaceKind::Color(UNIT),
        shown: |m| m.transmission > 0.0,
        get: |m| SurfaceValue::Color(m.transmission_color),
        hint: no_hint,
        put: |m, v| m.transmission_color = color(v),
    },
];

/// What an imported `KHR_materials_ior` `ior` of 0 becomes (#1998): the extension's
/// infinite ior, whose dielectric F0 is `min(specular_color, 1) × specular`, a ior the
/// table's range cannot hold. So it imports as glass's ior with its `specular_color`
/// raised by this factor, `1 / 0.04`: the same F0 (`MaterialAsset::dielectric_f0`),
/// and a refraction that stays defined.
const INFINITE_IOR_TINT: f32 = 25.0;

/// Write an imported glTF's `ior`, `specular` and `specular_color` (#1998) into `m`
/// through their rows, so an import lands in the ranges the Inspector and
/// `Material.Get*` report, as an authored value does: a NaN or out-of-range value is
/// clamped, and an `ior` of 0 is mapped first (to 1.5, its colour times 25).
pub fn import_reflectance(m: &mut MaterialAsset, ior: f32, specular: f32, color: [f32; 3]) {
    let (ior, color) = if ior == 0.0 {
        (1.5, color.map(|c| c * INFINITE_IOR_TINT))
    } else {
        (ior, color)
    };
    for (key, value) in [
        ("ior", SurfaceValue::Number(ior)),
        ("specular", SurfaceValue::Number(specular)),
        ("specular_color", SurfaceValue::Color(color)),
    ] {
        let Some(field) = SURFACE_FIELDS.iter().find(|f| f.key == key) else {
            continue;
        };
        if let Ok(value) = field.check(value) {
            (field.put)(m, value);
        }
    }
}

#[cfg(test)]
#[path = "surface_tests.rs"]
mod tests;
