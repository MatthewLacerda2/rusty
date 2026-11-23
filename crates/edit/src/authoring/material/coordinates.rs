use super::field::{no_hint, SurfaceField, SurfaceKind, SurfaceValue};
use super::surface::{choice, number};
use rusty_components::{MaterialAsset, UvMapping};

pub use rusty_scene::rules::material::{
    BLEND_SHARPNESS_RANGE, HEIGHT_SCALE_RANGE, HEIGHT_STEPS_RANGE, MAP_COORD_RANGE,
    WORLD_SIZE_RANGE,
};

fn pair(value: SurfaceValue) -> [f32; 2] {
    match value {
        SurfaceValue::Vec2(v) => v,
        _ => [0.0; 2],
    }
}

/// Tiling's hover text: under `UV0` it scales from the image's bottom edge, as Unity's does.
fn tiling_hint(m: &MaterialAsset) -> Option<&'static str> {
    Some(match m.uv_mapping {
        UvMapping::Uv0 => {
            "Unity's Tiling: repeats across the mesh's UVs, scaled from the image's bottom edge."
        }
        UvMapping::Planar | UvMapping::Triplanar => {
            "Repeats across World Size metres of the world projection."
        }
    })
}

/// Offset's sign under `UV0` is Unity's, on a world projection HDRP's.
fn offset_hint(m: &MaterialAsset) -> Option<&'static str> {
    Some(match m.uv_mapping {
        UvMapping::Uv0 => {
            "Unity's Offset, in repeats: +y moves the texture down the surface, +x toward \
             u = 0. Unity numbers carry over unchanged."
        }
        UvMapping::Planar | UvMapping::Triplanar => {
            "HDRP's Offset, in repeats of World Size: +x moves the texture toward world -X \
             (-Z on a wall facing X), +y toward world -Z on a floor and down on a wall."
        }
    })
}

fn world_mapped(m: &MaterialAsset) -> bool {
    m.uv_mapping != UvMapping::Uv0
}

pub static COORDINATE_FIELDS: &[SurfaceField] = &[
    SurfaceField {
        key: "uv_mapping",
        api: "UvMapping",
        label: "UV Mapping",
        kind: SurfaceKind::Choice(|| UvMapping::ALL.map(UvMapping::name).to_vec()),
        shown: |_| true,
        get: |m| SurfaceValue::Text(m.uv_mapping.name().to_owned()),
        hint: no_hint,
        put: |m, v| m.uv_mapping = choice(v, UvMapping::parse),
    },
    SurfaceField {
        key: "tiling",
        api: "Tiling",
        label: "Tiling",
        kind: SurfaceKind::Vec2(MAP_COORD_RANGE),
        shown: |_| true,
        get: |m| SurfaceValue::Vec2(m.tiling),
        hint: tiling_hint,
        put: |m, v| m.tiling = pair(v),
    },
    SurfaceField {
        key: "offset",
        api: "Offset",
        label: "Offset",
        kind: SurfaceKind::Vec2(MAP_COORD_RANGE),
        shown: |_| true,
        get: |m| SurfaceValue::Vec2(m.offset),
        hint: offset_hint,
        put: |m, v| m.offset = pair(v),
    },
    SurfaceField {
        key: "world_size",
        api: "WorldSize",
        label: "World Size (m)",
        kind: SurfaceKind::Number(WORLD_SIZE_RANGE),
        shown: world_mapped,
        get: |m| SurfaceValue::Number(m.world_size),
        hint: no_hint,
        put: |m, v| m.world_size = number(v),
    },
    SurfaceField {
        key: "blend_sharpness",
        api: "BlendSharpness",
        label: "Blend Sharpness",
        kind: SurfaceKind::Number(BLEND_SHARPNESS_RANGE),
        shown: |m| m.uv_mapping == UvMapping::Triplanar,
        get: |m| SurfaceValue::Number(m.blend_sharpness),
        hint: no_hint,
        put: |m, v| m.blend_sharpness = number(v),
    },
    SurfaceField {
        key: "height_scale",
        api: "HeightScale",
        label: "Height Scale",
        // HDRP's *Amplitude*, as a fraction of one repeat's width.
        kind: SurfaceKind::Number(HEIGHT_SCALE_RANGE),
        shown: |m| m.height_map.is_some(),
        get: |m| SurfaceValue::Number(m.height_scale),
        hint: |_| {
            Some("How deep the height map's black lies, as a fraction of one repeat's width.")
        },
        put: |m, v| m.height_scale = number(v),
    },
    SurfaceField {
        key: "height_min_steps",
        api: "HeightMinSteps",
        label: "Minimum Steps",
        kind: SurfaceKind::Number(HEIGHT_STEPS_RANGE),
        shown: |m| m.height_map.is_some(),
        get: |m| SurfaceValue::Number(m.height_min_steps as f32),
        hint: |_| Some(STEPS_HINT),
        put: |m, v| m.height_min_steps = steps(v),
    },
    SurfaceField {
        key: "height_max_steps",
        api: "HeightMaxSteps",
        label: "Maximum Steps",
        kind: SurfaceKind::Number(HEIGHT_STEPS_RANGE),
        shown: |m| m.height_map.is_some(),
        get: |m| SurfaceValue::Number(m.height_max_steps as f32),
        hint: |_| Some(STEPS_HINT),
        put: |m, v| m.height_max_steps = steps(v),
    },
];

const STEPS_HINT: &str =
    "HDRP's pixel-displacement steps: the march takes the minimum head on and \
     the maximum at a grazing view. Raise them for a deep relief of fine features; each step \
     is one more height-map read on every pixel the material covers.";

/// A step count, whole: `check` has already clamped it to its range.
fn steps(v: SurfaceValue) -> u32 {
    number(v).round() as u32
}

#[cfg(test)]
mod tests {
    use super::super::field::{set_surface, set_surfaces, surface_field, SurfaceField};
    use super::super::MaterialLibrary;
    use super::*;

    fn one() -> MaterialLibrary {
        MaterialLibrary::from([("k".to_string(), MaterialAsset::default())])
    }

    fn field(name: &str) -> &'static SurfaceField {
        surface_field(name).expect("a coordinate field")
    }

    #[test]
    fn a_minimum_step_count_above_the_maximum_is_refused_in_either_order() {
        let mut materials = one();
        let (min, max) = (field("HeightMinSteps"), field("HeightMaxSteps"));
        let err = set_surface(&mut materials, "k", min, SurfaceValue::Number(20.0)).unwrap_err();
        assert!(err.contains("raise the maximum first"), "{err}");
        assert_eq!(materials["k"].height_min_steps, 5, "nothing written");
        assert!(set_surface(&mut materials, "k", max, SurfaceValue::Number(4.0)).is_err());
        let raise = [
            (min, SurfaceValue::Number(32.0)),
            (max, SurfaceValue::Number(64.0)),
        ];
        set_surfaces(&mut materials, "k", raise).unwrap();
        let k = &materials["k"];
        assert_eq!((k.height_min_steps, k.height_max_steps), (32, 64));
        set_surface(&mut materials, "k", max, SurfaceValue::Number(99.5)).unwrap();
        assert_eq!(
            materials["k"].height_max_steps, 64,
            "clamped to HDRP's limit"
        );
    }

    #[test]
    fn an_inverted_pair_from_a_file_still_takes_other_fields() {
        let mut materials = one();
        let m = materials.get_mut("k").unwrap();
        (m.height_min_steps, m.height_max_steps) = (30, 10);
        set_surface(
            &mut materials,
            "k",
            field("HeightScale"),
            SurfaceValue::Number(0.1),
        )
        .unwrap();
        assert_eq!(materials["k"].height_scale, 0.1);
    }
}
