//! src/shadergen/gameplay_tests.rs — the gameplay surface blocks (#401): each one
//! composes against the real forward base, and `uv_scroll` moves the UVs at the top
//! of `fs_main` — before any map is sampled — only when a recipe holds it.

use std::collections::BTreeMap;

use super::assemble::assemble;
use super::recipe::{BlockSel, PassKind, ShaderRecipe};
use super::validate::validate;

const ENGINE_SHADERS: &str = "assets/shaders";
const NEW: [&str; 4] = ["pulse_glow", "uv_scroll", "triplanar_detail", "hologram"];

fn surface(ids: &[&str]) -> String {
    let recipe = ShaderRecipe {
        pass: PassKind::Surface,
        name: "t401".into(),
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
fn each_gameplay_block_composes_alone() {
    for id in NEW {
        validate(ENGINE_SHADERS, &surface(&[id])).unwrap_or_else(|e| panic!("{id}: {e}"));
    }
}

#[test]
fn gameplay_blocks_compose_in_one_chain_with_the_mask_blocks() {
    let mut ids = NEW.to_vec();
    ids.extend(["dissolve", "hit_flash", "uv_scroll"]);
    validate(ENGINE_SHADERS, &surface(&ids)).expect("one chain composes");
}

#[test]
fn uv_scroll_moves_the_uvs_before_any_map_is_sampled() {
    let wgsl = surface(&["toon_ramp", "uv_scroll"]);
    let opened = "fn fs_main(in_raw: VertexOutput) -> @location(0) vec4<f32> {\n    var in = in_raw;\n    in.tex_coords = uv_uv_scroll(in.tex_coords, in, uv_scroll_1_speed);";
    assert!(wgsl.contains(opened), "{wgsl}");
    let at = wgsl.find(opened).unwrap();
    assert!(
        !wgsl[..at].contains("fn fs_main"),
        "only the entry point is opened"
    );
    assert!(wgsl[at..].contains("textureSample(t_diffuse, s_diffuse, in.tex_coords)"));
    // A UV block never joins the color chain.
    assert!(!wgsl.contains("uv_uv_scroll(srf_"));
    assert!(wgsl.contains("srf_toon_ramp(lighting_color, in, toon_ramp_0_steps)"));
}

#[test]
fn without_a_uv_block_fs_main_is_untouched() {
    let wgsl = surface(&["pulse_glow", "hologram"]);
    assert!(!wgsl.contains("in_raw"));
    assert!(wgsl.contains("fn fs_main(in: VertexOutput)"));
}

#[test]
fn triplanar_reads_the_mask_slot_and_the_rest_do_not() {
    assert!(surface(&["triplanar_detail"]).contains("var t_mask"));
    assert!(!surface(&["pulse_glow", "uv_scroll", "hologram"]).contains("var t_mask"));
}
