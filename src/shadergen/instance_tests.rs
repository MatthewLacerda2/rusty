//! src/shadergen/instance_tests.rs — blocks as instances (#393) and strict params
//! (#395): a repeated block composes with each instance keeping its own params, and
//! an undeclared or mis-shaped param is an assemble error naming the culprit.

use super::assemble::assemble;
use super::blocks::catalog;
use super::recipe::{BlockSel, ParamValue, PassKind, ShaderRecipe};
use super::validate::validate;

const ENGINE_SHADERS: &str = "assets/shaders";

fn sel(id: &str, params: &[(&str, ParamValue)]) -> BlockSel {
    let params = params
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    BlockSel {
        id: id.into(),
        params,
    }
}

fn recipe(pass: PassKind, blocks: Vec<BlockSel>) -> ShaderRecipe {
    ShaderRecipe {
        pass,
        name: "t".into(),
        blocks,
    }
}

/// Assemble against the real surface base, so the output is what a bake validates.
fn assemble_real(r: &ShaderRecipe) -> Result<String, String> {
    let base = std::fs::read_to_string(format!("{ENGINE_SHADERS}/shader.wgsl")).unwrap();
    assemble(r, &base)
}

#[test]
fn a_repeated_block_keeps_per_instance_params_and_one_helper() {
    let r = recipe(
        PassKind::Postfx,
        vec![
            sel(
                "tint",
                &[("color", ParamValue::Vector(vec![1.0, 0.5, 0.25]))],
            ),
            sel("vignette", &[]),
            sel("tint", &[("color", ParamValue::Scalar(0.5))]),
        ],
    );
    let wgsl = assemble_real(&r).unwrap();
    assert!(wgsl.contains("const tint_0_color: vec3<f32> = vec3<f32>(1.0, 0.5, 0.25);"));
    assert!(wgsl.contains("const tint_2_color: vec3<f32> = vec3<f32>(0.5, 0.5, 0.5);"));
    assert_eq!(wgsl.matches("fn pfx_tint(").count(), 1, "{wgsl}");
    assert!(wgsl.contains("pfx_tint(pfx_vignette(pfx_tint(color, uv, tint_0_color)"));
    validate(ENGINE_SHADERS, &wgsl).expect("repeated block composes");
    assert_eq!(wgsl, assemble_real(&r).unwrap(), "deterministic");
}

#[test]
fn every_catalogued_block_twice_composes() {
    for pass in [PassKind::Surface, PassKind::Postfx] {
        let blocks = catalog(pass)
            .iter()
            .flat_map(|b| [sel(b.id, &[]), sel(b.id, &[])]);
        let wgsl = assemble_real(&recipe(pass, blocks.collect())).unwrap();
        validate(ENGINE_SHADERS, &wgsl)
            .unwrap_or_else(|e| panic!("{} with every block twice: {e}", pass.tag()));
    }
}

#[test]
fn an_undeclared_param_names_the_block_key_and_declared_params() {
    let r = recipe(
        PassKind::Postfx,
        vec![sel("vignette", &[("strenght", ParamValue::Scalar(1.0))])],
    );
    let err = assemble(&r, "").unwrap_err();
    for part in [
        "\"vignette\"",
        "blocks[0]",
        "\"strenght\"",
        "strength, radius",
    ] {
        assert!(err.contains(part), "missing {part}: {err}");
    }
}

#[test]
fn a_param_of_the_wrong_arity_is_refused() {
    let array_for_scalar = sel("exposure", &[("stops", ParamValue::Vector(vec![1.0]))]);
    let short_vector = sel("tint", &[("color", ParamValue::Vector(vec![1.0, 0.5]))]);
    for (block, want) in [
        (array_for_scalar, "expects a number"),
        (short_vector, "expects 3"),
    ] {
        let err = assemble(&recipe(PassKind::Postfx, vec![block]), "").unwrap_err();
        assert!(err.contains(want), "{err}");
    }
}

#[test]
fn an_unknown_recipe_or_block_key_is_refused() {
    let bad_block =
        r#"{ "pass": "postfx", "name": "t", "blocks": [{ "id": "tint", "param": {} }] }"#;
    let bad_recipe = r#"{ "pass": "postfx", "name": "t", "block": [] }"#;
    assert!(ShaderRecipe::from_json(bad_block)
        .unwrap_err()
        .contains("param"));
    assert!(ShaderRecipe::from_json(bad_recipe)
        .unwrap_err()
        .contains("block"));
}
