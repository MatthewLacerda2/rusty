//! The transparent-surface field table: every row reads back what it wrote, and
//! each kind refuses or clamps what it cannot hold.

use super::*;
use crate::components::BlendMode;

fn one() -> MaterialLibrary {
    MaterialLibrary::from([("k".to_string(), MaterialAsset::default())])
}

fn field(name: &str) -> &'static SurfaceField {
    surface_field(name).expect("a surface field")
}

#[test]
fn every_row_reads_back_what_it_wrote() {
    for f in material_fields() {
        let mut materials = one();
        let value = match f.kind {
            SurfaceKind::Choice(names) => SurfaceValue::Text(names().last().unwrap().to_string()),
            SurfaceKind::Toggle => SurfaceValue::Bool(!matches!(
                (f.get)(&materials["k"]),
                SurfaceValue::Bool(true)
            )),
            SurfaceKind::Number(ref r) => SurfaceValue::Number(*r.end()),
            SurfaceKind::Color => SurfaceValue::Color([0.25, 0.5, 0.75]),
            SurfaceKind::Vec2(ref r) => SurfaceValue::Vec2([*r.start(), 2.5]),
        };
        set_surface(&mut materials, "k", f, value.clone()).unwrap();
        assert_eq!((f.get)(&materials["k"]), value, "{}", f.key);
    }
}

#[test]
fn rows_are_found_by_key_or_api_name_and_unique() {
    assert_eq!(field("blend_mode").api, "BlendMode");
    assert_eq!(field("receiveshadows").key, "receive_shadows");
    let all: Vec<_> = material_fields().collect();
    for (i, f) in all.iter().enumerate() {
        let same = all[..i].iter().any(|g| g.key == f.key || g.api == f.api);
        assert!(!same, "{} is listed twice", f.key);
    }
}

#[test]
fn a_choice_parses_in_any_case_and_refuses_an_unknown_name() {
    let mut materials = one();
    let blend = field("blend_mode");
    set_surface(
        &mut materials,
        "k",
        blend,
        SurfaceValue::Text("additive".into()),
    )
    .unwrap();
    assert_eq!(materials["k"].blend_mode, BlendMode::Additive);
    let err = set_surface(
        &mut materials,
        "k",
        blend,
        SurfaceValue::Text("glow".into()),
    );
    assert!(err.unwrap_err().contains("Multiply"), "names the choices");
    assert_eq!(materials["k"].blend_mode, BlendMode::Additive, "unchanged");
}

#[test]
fn a_value_of_the_wrong_kind_is_refused() {
    let mut materials = one();
    let shadows = field("receive_shadows");
    let err = set_surface(&mut materials, "k", shadows, SurfaceValue::Number(0.0));
    assert!(err.unwrap_err().contains("boolean"));
    assert!(materials["k"].receive_shadows);
}

#[test]
fn the_defaults_preserve_specular_and_receive_shadows() {
    let m = MaterialAsset::default();
    assert_eq!(m.blend_mode, BlendMode::Alpha);
    assert!(m.preserve_specular && m.receive_shadows);
    assert!(BlendMode::Alpha.preserves_specular());
    assert!(!BlendMode::Multiply.preserves_specular());
    assert!(!BlendMode::Premultiply.preserves_specular());
}

#[test]
fn a_pair_is_clamped_per_component() {
    let mut materials = one();
    let tiling = SurfaceValue::Vec2([5000.0, -3.0]);
    set_surface(&mut materials, "k", field("Tiling"), tiling).unwrap();
    assert_eq!(materials["k"].tiling, [1000.0, -3.0]);
    let err = set_surface(
        &mut materials,
        "k",
        field("offset"),
        SurfaceValue::Number(1.0),
    );
    assert!(err.unwrap_err().contains("{x, y}"));
}
