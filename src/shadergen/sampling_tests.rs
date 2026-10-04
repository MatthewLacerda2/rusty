//! src/shadergen/sampling_tests.rs — the sampling / animated postfx blocks (#402)
//! chain with the grades and keep their declared defaults. Each block composing
//! alone is `tests::every_catalogued_block_bakes_into_a_composing_module`; the
//! pixel behaviour is pinned by `tests/gpu/postfx_blocks_screenshot.rs`.

use std::collections::BTreeMap;

use super::assemble::assemble;
use super::recipe::{BlockSel, PassKind, ShaderRecipe};
use super::validate::validate;

const ENGINE_SHADERS: &str = "assets/shaders";

fn recipe(pass: PassKind, ids: &[&str]) -> ShaderRecipe {
    ShaderRecipe {
        pass,
        name: "t402".into(),
        blocks: ids
            .iter()
            .map(|id| BlockSel {
                id: id.to_string(),
                params: BTreeMap::new(),
            })
            .collect(),
    }
}

#[test]
fn sampling_blocks_chain_after_grades() {
    // A sampling block reads the module input, so it composes anywhere in a chain.
    let ids = [
        "tint",
        "chromatic_aberration",
        "sharpen",
        "radial_blur",
        "film_grain",
        "damage_vignette",
    ];
    let wgsl = assemble(&recipe(PassKind::Postfx, &ids), "").unwrap();
    validate(ENGINE_SHADERS, &wgsl).expect("a mixed chain composes");
    assert!(wgsl.contains(
        "pfx_chromatic_aberration(pfx_tint(color, uv, tint_0_color), uv, shader_params.v[0].x)"
    ));
}

#[test]
fn radial_blur_center_defaults_to_the_screen_centre() {
    let wgsl = assemble(&recipe(PassKind::Postfx, &["radial_blur"]), "").unwrap();
    assert!(wgsl.contains("const radial_blur_0_center: vec2<f32> = vec2<f32>(0.5, 0.5);"));
}
