//! src/shadergen/texture_tests.rs — the extra texture socket (#400): a surface
//! variant declares the `mask` slot only when a block reads it, once however many
//! do, and the declaring module composes against the real forward base.

use std::collections::BTreeMap;

use super::assemble::assemble;
use super::recipe::{BlockSel, PassKind, ShaderRecipe};
use super::validate::validate;

const ENGINE_SHADERS: &str = "assets/shaders";
const MASK_DECL: &str = "@group(2) @binding(7) var t_mask: texture_2d<f32>;";

fn surface(ids: &[&str]) -> String {
    let recipe = ShaderRecipe {
        pass: PassKind::Surface,
        name: "t400".into(),
        blocks: ids
            .iter()
            .map(|id| BlockSel {
                id: id.to_string(),
                params: BTreeMap::new(),
            })
            .collect(),
    };
    let base = std::fs::read_to_string(format!("{ENGINE_SHADERS}/shader.wgsl")).unwrap();
    assemble(&recipe, &base).unwrap()
}

#[test]
fn a_variant_without_a_mask_block_declares_no_mask() {
    let wgsl = surface(&["toon_ramp", "hit_flash"]);
    assert!(!wgsl.contains("t_mask"), "plain contract kept");
}

#[test]
fn mask_blocks_declare_the_slot_once_and_compose() {
    let wgsl = surface(&["dissolve", "detail_overlay", "hit_flash"]);
    assert_eq!(wgsl.matches(MASK_DECL).count(), 1, "{wgsl}");
    validate(ENGINE_SHADERS, &wgsl).expect("mask blocks compose in one chain");
}

#[test]
fn dissolve_amount_is_runtime_so_a_script_burns_the_surface_away() {
    let wgsl = surface(&["dissolve"]);
    assert!(wgsl.contains("srf_dissolve(lighting_color, in, shader_params.v[0].x"));
    assert!(wgsl.contains("discard;"));
}
