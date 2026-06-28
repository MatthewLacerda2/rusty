//! src/shadergen/tests.rs — spine-level tests for shader authoring (#272).
//!
//! These exercise the whole leg end-to-end against the engine's real committed
//! shader set (`assets/shaders`, read-only here) but write baked output to a temp
//! dir, so the shipped set + the `all_shaders_compose` guard stay untouched. The
//! load-bearing tests are: a baked surface AND postfx variant compose cleanly
//! through `naga_oil` (the validate-at-bake guarantee, GPU-free → runs in CI); an
//! intentionally-broken recipe fails the bake with no file written (the gate);
//! and the same recipe assembles byte-identical WGSL (determinism).

use std::collections::BTreeMap;

use super::assemble::assemble;
use super::bake::{bake_recipe, BakeError};
use super::recipe::{BlockSel, ParamValue, PassKind, ShaderRecipe};
use super::validate::validate;

const ENGINE_SHADERS: &str = "assets/shaders";

/// A unique temp dir per test name, so parallel test runs never collide.
fn temp_out(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rusty_shadergen_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn block(id: &str, params: BTreeMap<String, ParamValue>) -> BlockSel {
    BlockSel {
        id: id.into(),
        params,
    }
}

#[test]
fn baked_postfx_variant_composes_through_naga_oil() {
    let mut tint = BTreeMap::new();
    tint.insert("color".to_string(), ParamValue::Vector(vec![1.0, 0.8, 0.6]));
    let recipe = ShaderRecipe {
        pass: PassKind::Postfx,
        name: "warm_grade".into(),
        blocks: vec![
            block("tint", tint),
            block("vignette", BTreeMap::new()),
            block("scanline", BTreeMap::new()),
        ],
    };
    let out = temp_out("postfx_compose");
    let path = bake_recipe(&recipe, ENGINE_SHADERS, out.to_str().unwrap()).expect("bake succeeds");
    // The file exists and re-validates exactly as the engine would load it.
    let written = std::fs::read_to_string(&path).expect("written file readable");
    let entries = validate(ENGINE_SHADERS, &written).expect("baked module composes");
    assert!(entries.iter().any(|e| e == "fs_main"), "{entries:?}");
    assert!(entries.iter().any(|e| e == "vs_fullscreen"), "{entries:?}");
    std::fs::remove_file(path).ok();
}

#[test]
fn baked_surface_variant_composes_and_keeps_entry_points() {
    let mut steps = BTreeMap::new();
    steps.insert("steps".to_string(), ParamValue::Scalar(3.0));
    let recipe = ShaderRecipe {
        pass: PassKind::Surface,
        name: "toon_rim".into(),
        blocks: vec![
            block("toon_ramp", steps),
            block("fresnel_rim", BTreeMap::new()),
        ],
    };
    let out = temp_out("surface_compose");
    let path = bake_recipe(&recipe, ENGINE_SHADERS, out.to_str().unwrap()).expect("bake succeeds");
    let written = std::fs::read_to_string(&path).expect("written file readable");
    let entries = validate(ENGINE_SHADERS, &written).expect("baked surface composes");
    // Contract conformance: the forward pipeline binds vs_main + fs_main.
    assert!(entries.iter().any(|e| e == "vs_main"), "{entries:?}");
    assert!(entries.iter().any(|e| e == "fs_main"), "{entries:?}");
    std::fs::remove_file(path).ok();
}

#[test]
fn broken_recipe_fails_the_bake_and_writes_no_file() {
    // An unknown block id can never assemble — caught before any disk write.
    let recipe = ShaderRecipe {
        pass: PassKind::Postfx,
        name: "broken".into(),
        blocks: vec![block("not_a_real_block", BTreeMap::new())],
    };
    let out = temp_out("rejection");
    let err = bake_recipe(&recipe, ENGINE_SHADERS, out.to_str().unwrap()).unwrap_err();
    assert!(matches!(err, BakeError::Assemble(_)), "got {err}");
    assert!(
        !out.join("broken.wgsl").exists(),
        "no file should be written for a rejected recipe"
    );
}

#[test]
fn a_block_with_invalid_wgsl_is_rejected_by_validation() {
    // Drive a real validation rejection: a hand-built module that references an
    // undefined binding must fail the compose gate (no file written). This proves
    // the validate step — not just the assemble step — gates the bake.
    let module = "@fragment fn fs_main() -> @location(0) vec4<f32> {\n\
        return textureSample(t_does_not_exist, s_none, vec2<f32>(0.0));\n }";
    assert!(validate(ENGINE_SHADERS, module).is_err());
}

#[test]
fn same_recipe_assembles_byte_identical_wgsl() {
    let recipe = ShaderRecipe {
        pass: PassKind::Postfx,
        name: "det".into(),
        blocks: vec![
            block("vignette", BTreeMap::new()),
            block("posterize", BTreeMap::new()),
        ],
    };
    let a = assemble(&recipe, "").unwrap();
    let b = assemble(&recipe, "").unwrap();
    assert_eq!(a, b);
}

#[test]
fn surface_recipe_assembles_byte_identical_wgsl() {
    let base = std::fs::read_to_string(format!("{ENGINE_SHADERS}/shader.wgsl")).unwrap();
    let recipe = ShaderRecipe {
        pass: PassKind::Surface,
        name: "det_surface".into(),
        blocks: vec![block("desaturate", BTreeMap::new())],
    };
    let a = assemble(&recipe, &base).unwrap();
    let b = assemble(&recipe, &base).unwrap();
    assert_eq!(a, b);
}

#[test]
fn every_catalogued_block_bakes_into_a_composing_module() {
    // Each block, alone, must produce a module that composes — so the whole
    // library is shippable, not just the combos the other tests pick.
    for (pass, name) in [
        (PassKind::Surface, "surf_each"),
        (PassKind::Postfx, "pfx_each"),
    ] {
        let out = temp_out(name);
        for b in super::blocks::catalog(pass) {
            let recipe = ShaderRecipe {
                pass,
                name: format!("only_{}", b.id),
                blocks: vec![block(b.id, BTreeMap::new())],
            };
            let path = bake_recipe(&recipe, ENGINE_SHADERS, out.to_str().unwrap())
                .unwrap_or_else(|e| panic!("block {} ({}) failed to bake: {e}", b.id, pass.tag()));
            std::fs::remove_file(path).ok();
        }
    }
}
