use std::collections::BTreeMap;

use super::assemble::assemble;
use super::recipe::{BlockSel, PassKind, ShaderRecipe};
use super::validate::validate;
use crate::engine_shader_dir;

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
    let ids = [
        "tint",
        "sharpen",
        "radial_blur",
        "scanline",
        "damage_vignette",
    ];
    let wgsl = assemble(&recipe(PassKind::Postfx, &ids), "").unwrap();
    validate(engine_shader_dir(), &wgsl).expect("a mixed chain composes");
    assert!(
        wgsl.contains("pfx_sharpen(pfx_tint(color, uv, tint_0_color), uv, shader_params.v[0].x)")
    );
}

#[test]
fn radial_blur_center_defaults_to_the_screen_centre() {
    let wgsl = assemble(&recipe(PassKind::Postfx, &["radial_blur"]), "").unwrap();
    assert!(wgsl.contains("const radial_blur_0_center: vec2<f32> = vec2<f32>(0.5, 0.5);"));
}
