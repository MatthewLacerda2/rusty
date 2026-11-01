//! src/shadergen/crt_tests.rs — the CRT postfx blocks (#1679): `barrel`'s uv stage
//! warps where the frame is sampled, and `phosphor_mask`'s `kind` is a named choice.
//! The pixels are pinned by `tests/gpu/postfx/crt_screenshot.rs`.

use super::assemble::assemble;
use super::recipe::{BlockSel, ParamValue, PassKind, ShaderRecipe};
use super::validate::validate;

const ENGINE_SHADERS: &str = "engine/shaders";

fn recipe(blocks: Vec<BlockSel>) -> ShaderRecipe {
    ShaderRecipe {
        pass: PassKind::Postfx,
        name: "t1679".into(),
        blocks,
    }
}

fn sel(id: &str, params: &[(&str, ParamValue)]) -> BlockSel {
    BlockSel {
        id: id.into(),
        params: params
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect(),
    }
}

#[test]
fn barrel_moves_the_sample_before_any_colour_block_and_blacks_out_the_rest() {
    let wgsl = assemble(&recipe(vec![sel("tint", &[]), sel("barrel", &[])]), "").unwrap();
    validate(ENGINE_SHADERS, &wgsl).expect("barrel + tint composes");
    // Its place in the recipe doesn't matter: the uv chain runs before the sample.
    assert!(
        wgsl.contains("let uv = uv_barrel(in.uv, shader_params.v[0].x, barrel_1_corner_radius);")
    );
    assert!(wgsl.contains("color = pfx_tint(color, uv, tint_0_color);"));
    assert!(wgsl.contains("return vec4<f32>(color * on_screen(uv), 1.0);"));
}

#[test]
fn a_recipe_without_a_uv_block_assembles_as_before() {
    let wgsl = assemble(&recipe(vec![sel("scanline", &[])]), "").unwrap();
    assert!(wgsl.contains("let uv = in.uv;\n"));
    assert!(wgsl.contains("return vec4<f32>(color, 1.0);"));
    assert!(!wgsl.contains("on_screen"));
}

#[test]
fn phosphor_kind_is_named_and_baked_as_its_index() {
    for (i, kind) in ["aperture", "shadow", "slot"].iter().enumerate() {
        let named = sel(
            "phosphor_mask",
            &[("kind", ParamValue::Name((*kind).into()))],
        );
        let wgsl = assemble(&recipe(vec![named]), "").unwrap();
        validate(ENGINE_SHADERS, &wgsl).unwrap_or_else(|e| panic!("{kind}: {e}"));
        assert!(wgsl.contains(&format!("const phosphor_mask_0_kind: f32 = {i}.0;")));
        assert!(wgsl.contains("pfx_phosphor_mask(color, in.pos.xy, phosphor_mask_0_kind"));
    }
    let default = assemble(&recipe(vec![sel("phosphor_mask", &[])]), "").unwrap();
    assert!(default.contains("const phosphor_mask_0_kind: f32 = 0.0;"));
}

#[test]
fn a_choice_takes_only_its_names_and_a_number_takes_no_name() {
    let refused = |params: &[(&str, ParamValue)]| {
        assemble(&recipe(vec![sel("phosphor_mask", params)]), "").unwrap_err()
    };
    let typo = refused(&[("kind", ParamValue::Name("apperture".into()))]);
    assert!(
        typo.contains("expects one of aperture, shadow, slot"),
        "{typo}"
    );
    let number = refused(&[("kind", ParamValue::Scalar(1.0))]);
    assert!(number.contains("expects one of"), "{number}");
    let named = refused(&[("scale", ParamValue::Name("big".into()))]);
    assert!(named.contains("expects a number, got \"big\""), "{named}");
}

#[test]
fn a_json_recipe_names_the_kind() {
    let json = r#"{"pass":"postfx","name":"crt","blocks":[{"id":"phosphor_mask","params":{"kind":"slot"}}]}"#;
    let parsed: ShaderRecipe = serde_json::from_str(json).unwrap();
    assert_eq!(
        parsed.blocks[0].params.get("kind"),
        Some(&ParamValue::Name("slot".into()))
    );
    let back: ShaderRecipe = serde_json::from_str(&parsed.to_json().unwrap()).unwrap();
    assert_eq!(back, parsed);
}
