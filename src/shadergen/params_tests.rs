//! Runtime shader params (#399): runtime block params become uniform slots instead
//! of `const`s, the layout resolves script names strictly, and a bake writes it.

use std::collections::BTreeMap;

use super::assemble::assemble_with_params;
use super::params::{ParamLayout, PARAM_SLOTS};
use super::recipe::{BlockSel, ParamValue, PassKind, ShaderRecipe};
use super::validate::validate;
use super::{bake_recipe, ENGINE_SHADER_DIR};

fn surface(blocks: &[(&str, &[(&str, ParamValue)])]) -> ShaderRecipe {
    let blocks = blocks
        .iter()
        .map(|(id, params)| BlockSel {
            id: id.to_string(),
            params: params
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        })
        .collect();
    ShaderRecipe {
        pass: PassKind::Surface,
        name: "t".into(),
        blocks,
    }
}

fn assemble(r: &ShaderRecipe) -> Result<(String, ParamLayout), String> {
    let base = std::fs::read_to_string(format!("{ENGINE_SHADER_DIR}/shader.wgsl")).unwrap();
    assemble_with_params(r, &base)
}

#[test]
fn runtime_params_read_the_uniform_and_baked_ones_stay_const() {
    let r = surface(&[
        ("toon_ramp", &[]),
        (
            "hit_flash",
            &[("color", ParamValue::Vector(vec![1.0, 0.0, 0.0]))],
        ),
    ]);
    let (wgsl, layout) = assemble(&r).unwrap();
    assert!(wgsl.contains("const toon_ramp_0_steps: f32 = 4.0;"));
    assert!(
        !wgsl.contains("const hit_flash_1_"),
        "runtime params are not baked"
    );
    assert!(wgsl.contains("@group(2) @binding(6) var<uniform> shader_params"));
    assert!(wgsl.contains("srf_hit_flash(srf_toon_ramp(lighting_color, in, toon_ramp_0_steps), in, shader_params.v[0].xyz, shader_params.v[1].x)"));
    validate(ENGINE_SHADER_DIR, &wgsl).expect("a runtime-param variant composes");
    assert_eq!(layout.names(), ["hit_flash.color", "hit_flash.amount"]);
    assert_eq!(
        layout.params[0].default,
        [1.0, 0.0, 0.0],
        "the recipe value is the default"
    );
    assert_eq!(assemble(&r).unwrap().0, wgsl, "deterministic");
}

#[test]
fn a_shader_with_no_runtime_params_declares_no_uniform() {
    let (wgsl, layout) = assemble(&surface(&[("toon_ramp", &[])])).unwrap();
    assert!(!wgsl.contains("shader_params"));
    assert!(layout.params.is_empty());
}

#[test]
fn names_resolve_short_indexed_and_refuse_the_rest() {
    let (_, one) = assemble(&surface(&[("toon_ramp", &[]), ("hit_flash", &[])])).unwrap();
    assert_eq!(one.resolve("hit_flash.amount").unwrap().slot, 1);
    assert_eq!(one.resolve("hit_flash.1.amount").unwrap().slot, 1);
    for (bad, want) in [
        ("toon_ramp.steps", "is baked"),
        ("hit_flash.amont", "not a runtime param"),
        ("hit_flash.0.amount", "not a runtime param"),
        (
            "hit_flash",
            "runtime params: hit_flash.color, hit_flash.amount",
        ),
    ] {
        let err = one.resolve(bad).unwrap_err();
        assert!(err.contains(want) && err.contains(bad), "{bad}: {err}");
    }

    let (_, two) = assemble(&surface(&[("tint", &[]), ("tint", &[])])).unwrap();
    assert_eq!(two.names(), ["tint.0.color", "tint.1.color"]);
    assert!(two.resolve("tint.color").unwrap_err().contains("ambiguous"));
    assert_eq!(two.resolve("tint.1.color").unwrap().slot, 1);
}

#[test]
fn coerce_broadcasts_one_number_and_refuses_a_wrong_count() {
    let (_, layout) = assemble(&surface(&[("hit_flash", &[])])).unwrap();
    let color = layout.resolve("hit_flash.color").unwrap();
    assert_eq!(layout.coerce(color, vec![0.5]).unwrap(), [0.5; 3]);
    assert!(layout
        .coerce(color, vec![1.0, 2.0])
        .unwrap_err()
        .contains("takes 3"));
}

#[test]
fn pack_lays_defaults_then_the_materials_values() {
    let (_, layout) = assemble(&surface(&[("hit_flash", &[])])).unwrap();
    let values = BTreeMap::from([
        ("hit_flash.amount".to_string(), vec![0.75]),
        ("gone.param".to_string(), vec![9.0]),
    ]);
    let packed = layout.pack(&values);
    assert_eq!(packed[0], [1.0, 1.0, 1.0, 0.0], "default white");
    assert_eq!(packed[1], [0.75, 0.0, 0.0, 0.0]);
    assert!(packed[2..].iter().all(|v| *v == [0.0; 4]));
}

#[test]
fn more_runtime_params_than_slots_is_an_error() {
    let blocks: Vec<(&str, &[(&str, ParamValue)])> = vec![("tint", &[]); PARAM_SLOTS + 1];
    assert!(assemble(&surface(&blocks))
        .unwrap_err()
        .contains("runtime params"));
}

#[test]
fn a_surface_bake_writes_the_layout_beside_the_module() {
    let dir = crate::test_temp::dir().join(format!("rusty_params_{}", std::process::id()));
    let out = dir.to_str().unwrap();
    let mut r = surface(&[("hit_flash", &[])]);
    r.name = "flash".into();
    let path = bake_recipe(&r, ENGINE_SHADER_DIR, out).unwrap();
    let layout = ParamLayout::read_beside(std::path::Path::new(&path)).unwrap();
    assert_eq!(layout.names(), ["hit_flash.color", "hit_flash.amount"]);
    // A module with no sidecar (baked before #399) has no runtime params.
    let bare = dir.join("old.wgsl");
    std::fs::write(&bare, "").unwrap();
    assert_eq!(
        ParamLayout::read_beside(&bare).unwrap(),
        ParamLayout::default()
    );
}
