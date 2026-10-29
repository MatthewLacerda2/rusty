//! The material asset's defaults and serde shape.

use super::*;

#[test]
fn defaults_are_opaque_and_fully_solid() {
    let m = MaterialAsset::default();
    assert_eq!(m.render_mode, RenderMode::Opaque);
    assert_eq!(m.alpha, 1.0);
    assert_eq!(m.alpha_cutoff, 0.5);
    assert!(!m.is_transparent());
}

#[test]
fn is_transparent_only_for_transparent_mode() {
    let with_mode = |mode| MaterialAsset {
        render_mode: mode,
        ..MaterialAsset::default()
    };
    assert!(!with_mode(RenderMode::Opaque).is_transparent());
    assert!(!with_mode(RenderMode::Cutout).is_transparent());
    assert!(with_mode(RenderMode::Transparent).is_transparent());
}

#[test]
fn transparency_fields_round_trip_through_serde() {
    let m = MaterialAsset {
        render_mode: RenderMode::Transparent,
        alpha: 0.25,
        alpha_cutoff: 0.7,
        ..MaterialAsset::default()
    };
    let json = serde_json::to_string(&m).expect("serialize");
    let back: MaterialAsset = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.render_mode, RenderMode::Transparent);
    assert_eq!(back.alpha, 0.25);
    assert_eq!(back.alpha_cutoff, 0.7);
}

#[test]
fn a_material_without_transparency_fields_loads_opaque() {
    // A hand-written material that names no render_mode/alpha/alpha_cutoff loads
    // through `#[serde(default)]` as a fully-opaque material.
    let json = r#"{ "base_color": [0.5, 0.5, 0.5], "metallic": 0.0, "roughness": 0.5 }"#;
    let m: MaterialAsset = serde_json::from_str(json).expect("a sparse material parses");
    assert_eq!(m.render_mode, RenderMode::Opaque);
    assert_eq!(m.alpha, 1.0);
    assert_eq!(m.alpha_cutoff, 0.5);
}

#[test]
fn shader_params_and_textures_round_trip_and_are_omitted_when_empty() {
    let m = MaterialAsset {
        shader: Some("enemy_hit".into()),
        shader_params: BTreeMap::from([("hit_flash.amount".into(), vec![0.5])]),
        shader_textures: BTreeMap::from([("mask".into(), "noise.png".into())]),
        ..MaterialAsset::default()
    };
    let back: MaterialAsset = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
    assert_eq!(back.shader_params, m.shader_params);
    assert_eq!(back.shader_textures, m.shader_textures);
    let plain = serde_json::to_string(&MaterialAsset::default()).unwrap();
    assert!(!plain.contains("shader_params"), "{plain}");
    assert!(!plain.contains("shader_textures"), "{plain}");
}

#[test]
fn map_coordinates_round_trip_and_default_when_absent() {
    let json = r#"{ "tiling": [4, 2], "uv_mapping": "Triplanar", "world_size": 2 }"#;
    let m: MaterialAsset = serde_json::from_str(json).expect("the keys parse");
    assert_eq!((m.tiling, m.offset), ([4.0, 2.0], [0.0, 0.0]));
    assert_eq!(m.uv_mapping, UvMapping::Triplanar);
    assert_eq!(m.blend_sharpness, 4.0, "an absent key keeps its default");
    let back: MaterialAsset = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
    assert_eq!(
        (back.tiling, back.uv_mapping, back.world_size),
        (m.tiling, m.uv_mapping, 2.0)
    );
    let plain: MaterialAsset = serde_json::from_str("{}").unwrap();
    assert_eq!(
        (plain.tiling, plain.uv_mapping),
        ([1.0, 1.0], UvMapping::Uv0)
    );
}
