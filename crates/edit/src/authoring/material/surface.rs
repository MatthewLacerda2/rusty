use super::field::{no_hint, SurfaceField, SurfaceKind, SurfaceValue};
use rusty_components::{
    BlendMode, MaterialAsset, RefractionModel, TaaReactive, SORTING_PRIORITY_RANGE,
};

fn transparent(m: &MaterialAsset) -> bool {
    m.is_transparent()
}

pub(super) fn always(_: &MaterialAsset) -> bool {
    true
}

pub(super) fn choice<T: Default>(value: SurfaceValue, parse: fn(&str) -> Option<T>) -> T {
    match value {
        SurfaceValue::Text(t) => parse(&t).unwrap_or_default(),
        _ => T::default(),
    }
}

fn boolean(value: SurfaceValue) -> bool {
    matches!(value, SurfaceValue::Bool(true))
}

pub(super) fn number(value: SurfaceValue) -> f32 {
    match value {
        SurfaceValue::Number(n) => n,
        _ => 0.0,
    }
}

fn color(value: SurfaceValue) -> [f32; 3] {
    match value {
        SurfaceValue::Color(c) => c,
        _ => [1.0; 3],
    }
}

/// The channel range of a colour that can only tint (`transmittance_color`,
/// `transmission_color`); `specular_color` can also raise, up to 25.
const UNIT: std::ops::RangeInclusive<f32> = 0.0..=1.0;

const PRIORITY: std::ops::RangeInclusive<f32> =
    -(SORTING_PRIORITY_RANGE as f32)..=SORTING_PRIORITY_RANGE as f32;

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
        key: "occlusion_strength",
        api: "OcclusionStrength",
        label: "Occlusion Strength",
        kind: SurfaceKind::Number(0.0..=1.0),
        shown: |m| m.occlusion_map.is_some(),
        get: |m| SurfaceValue::Number(m.occlusion_strength),
        hint: no_hint,
        put: |m, v| m.occlusion_strength = number(v),
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

/// What an imported `KHR_materials_ior` of 0 (infinite ior) becomes: glass's ior with
/// `specular_color` raised by `1 / 0.04`, the same F0 and a refraction that stays defined.
const INFINITE_IOR_TINT: f32 = 25.0;

pub fn import_reflectance(m: &mut MaterialAsset, ior: f32, specular: f32, color: [f32; 3]) {
    let (ior, color) = if ior == 0.0 {
        (1.5, color.map(|c| c * INFINITE_IOR_TINT))
    } else {
        (ior, color)
    };
    import_field(m, "ior", SurfaceValue::Number(ior));
    import_field(m, "specular", SurfaceValue::Number(specular));
    import_field(m, "specular_color", SurfaceValue::Color(color));
}

pub fn import_field(m: &mut MaterialAsset, key: &str, value: SurfaceValue) {
    let Some(field) = SURFACE_FIELDS.iter().find(|f| f.key == key) else {
        return;
    };
    if let Ok(value) = field.check(value) {
        (field.put)(m, value);
    }
}

#[cfg(test)]
#[path = "surface_tests.rs"]
mod tests;
