//! src/shadergen/ui_tests.rs — the `ui` pass (#427): every block composes against
//! the real `ui.wgsl`, the variant keeps both entry-point pairs and replaces only
//! `graphic()`, and its runtime params get uniform slots and a sidecar.

use std::collections::BTreeMap;

use super::assemble::{assemble, assemble_with_params};
use super::bake::base_source;
use super::blocks::catalog;
use super::recipe::{BlockSel, PassKind, ShaderRecipe};
use super::validate::validate;

const ENGINE_SHADERS: &str = "engine/shaders";

fn recipe(ids: &[&str]) -> ShaderRecipe {
    ShaderRecipe {
        pass: PassKind::Ui,
        name: "t427".into(),
        blocks: ids
            .iter()
            .map(|id| BlockSel {
                id: id.to_string(),
                params: BTreeMap::new(),
            })
            .collect(),
    }
}

fn ui(ids: &[&str]) -> String {
    let base = base_source(PassKind::Ui, ENGINE_SHADERS).unwrap();
    assemble(&recipe(ids), &base).unwrap()
}

#[test]
fn the_engine_ui_shader_carries_the_splice_point() {
    let base = base_source(PassKind::Ui, ENGINE_SHADERS).unwrap();
    assert!(base.contains(super::assemble::ui::GRAPHIC));
}

#[test]
fn each_ui_block_composes_alone_with_both_entry_point_pairs() {
    for block in catalog(PassKind::Ui) {
        let entries = validate(ENGINE_SHADERS, &ui(&[block.id]))
            .unwrap_or_else(|e| panic!("{}: {e}", block.id));
        for e in ["vs_main", "fs_main", "vs_world", "fs_world"] {
            assert!(entries.iter().any(|n| n == e), "{}: no {e}", block.id);
        }
    }
}

#[test]
fn every_ui_block_composes_in_one_chain_and_twice() {
    let mut ids: Vec<&str> = catalog(PassKind::Ui).iter().map(|b| b.id).collect();
    ids.push("glitch_slices");
    validate(ENGINE_SHADERS, &ui(&ids)).expect("one chain composes");
}

#[test]
fn only_graphic_is_replaced_and_the_chain_folds_over_shade() {
    let wgsl = ui(&["scanlines", "rgb_split"]);
    assert_eq!(wgsl.matches("fn graphic(").count(), 1);
    assert!(!wgsl.contains(super::assemble::ui::GRAPHIC));
    assert!(wgsl.contains(
        "return ui_rgb_split(ui_scanlines(shade(in), in, f, scanlines_0_spacing, shader_params.v[0].x, scanlines_0_speed), in, f, shader_params.v[1].x, rgb_split_1_angle);"
    ));
    // The world pass keeps its RectMask clip and Mask coverage around the custom colour.
    assert!(wgsl.contains("let c = graphic(in) * clip_coverage(in.canvas);"));
}

#[test]
fn an_empty_ui_recipe_draws_like_the_standard_shader() {
    let wgsl = ui(&[]);
    assert!(wgsl.contains("fn graphic(in: VertexOut) -> vec4<f32> {\n    let f = ui_frag(in);\n    return shade(in);\n}"));
    validate(ENGINE_SHADERS, &wgsl).expect("composes");
}

#[test]
fn ui_runtime_params_get_slots_by_canonical_name() {
    let base = base_source(PassKind::Ui, ENGINE_SHADERS).unwrap();
    let r = recipe(&["dissolve", "wipe", "wipe"]);
    let (_, layout) = assemble_with_params(&r, &base).unwrap();
    assert_eq!(
        layout.names(),
        ["dissolve.amount", "wipe.1.progress", "wipe.2.progress"]
    );
    assert!(layout.resolve("wipe.progress").is_err(), "ambiguous");
    let baked = layout.resolve("dissolve.scale").unwrap_err();
    assert!(baked.contains("is baked"), "{baked}");
}

#[test]
fn bake_writes_a_ui_module_and_its_sidecar() {
    let dir = std::env::temp_dir().join(format!("rusty_ui_bake_{}", std::process::id()));
    let out = dir.to_string_lossy().to_string();
    let path = super::bake_recipe(&recipe(&["noise_flicker"]), ENGINE_SHADERS, &out).unwrap();
    let sidecar = super::params::sidecar_path(std::path::Path::new(&path));
    let json = std::fs::read_to_string(sidecar).expect("sidecar written");
    assert!(json.contains("\"strength\""), "{json}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_crlf_checkout_of_the_base_still_splices() {
    let dir = std::env::temp_dir().join(format!("rusty_ui_crlf_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let base = base_source(PassKind::Ui, ENGINE_SHADERS).unwrap();
    std::fs::write(dir.join("ui.wgsl"), base.replace('\n', "\r\n")).unwrap();
    let crlf = base_source(PassKind::Ui, &dir.to_string_lossy()).unwrap();
    assert!(crlf.contains(super::assemble::ui::GRAPHIC));
    let _ = std::fs::remove_dir_all(dir);
}
