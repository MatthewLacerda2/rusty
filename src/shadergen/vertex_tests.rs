//! src/shadergen/vertex_tests.rs — the surface vertex stage (#1524): a vertex block
//! replaces the forward base's identity `surface_vertex` hook, and its variant
//! validates with every entry point a mesh is drawn through.

use std::collections::BTreeMap;

use super::assemble::{assemble_with_params, CUT_PREPASS, CUT_SHADOW};
use super::bake::base_source;
use super::recipe::{BlockSel, ParamValue, PassKind, ShaderRecipe};
use super::validate::validate;

const ENGINE_SHADERS: &str = "engine/shaders";
const IDENTITY: &str = "-> SurfaceVertex {\n    return v;\n}";

fn surface(ids: &[&str]) -> ShaderRecipe {
    let blocks = ids.iter().map(|id| BlockSel {
        id: (*id).into(),
        params: BTreeMap::new(),
    });
    ShaderRecipe {
        pass: PassKind::Surface,
        name: "waves".into(),
        blocks: blocks.collect(),
    }
}

fn baked(recipe: &ShaderRecipe) -> String {
    let base = base_source(PassKind::Surface, ENGINE_SHADERS).unwrap();
    assemble_with_params(recipe, &base).unwrap().0
}

#[test]
fn the_forward_base_hands_every_vertex_stage_the_identity_hook() {
    let base = base_source(PassKind::Surface, ENGINE_SHADERS).unwrap();
    assert!(base.contains(IDENTITY));
    // forward_vertex, vs_velocity's last-frame pose and vs_shadow: three callers.
    assert_eq!(base.matches("surface_vertex(world_vertex(").count(), 3);
    assert!(
        baked(&surface(&["tint"])).contains(IDENTITY),
        "colour blocks leave it"
    );
}

#[test]
fn vertex_waves_replaces_the_hook_and_validates_in_every_pass() {
    for ids in [
        &["vertex_waves"][..],
        &["vertex_waves", "vertex_waves"],
        &["uv_scroll", "vertex_waves", "dissolve", "fresnel_rim"],
    ] {
        let wgsl = baked(&surface(ids));
        assert!(!wgsl.contains(IDENTITY), "{ids:?}");
        let entries = validate(ENGINE_SHADERS, &wgsl).unwrap_or_else(|e| panic!("{ids:?}: {e}"));
        for entry in [
            "vs_main",
            "fs_main",
            "vs_velocity",
            "fs_velocity",
            "vs_shadow",
            CUT_PREPASS,
            CUT_SHADOW,
        ] {
            assert!(entries.iter().any(|e| e == entry), "{ids:?}: {entry}");
        }
    }
}

#[test]
fn the_chain_reads_amplitude_from_the_param_uniform_and_bakes_the_rest() {
    let mut recipe = surface(&["vertex_waves"]);
    recipe.blocks[0]
        .params
        .insert("count".into(), ParamValue::Scalar(5.0));
    let wgsl = baked(&recipe);
    assert!(wgsl.contains("return vtx_vertex_waves(v, time, shader_params.v[0].x,"));
    assert!(wgsl.contains("const vertex_waves_0_direction: vec2<f32> = vec2<f32>(1.0, 0.3);"));
    assert!(wgsl.contains("const vertex_waves_0_count: f32 = 5.0;"));
}

#[test]
fn two_vertex_blocks_chain_in_recipe_order() {
    let wgsl = baked(&surface(&["vertex_waves", "vertex_waves"]));
    assert!(
        wgsl.contains("return vtx_vertex_waves(vtx_vertex_waves(v, time, shader_params.v[0].x,")
    );
    assert_eq!(
        wgsl.matches("fn vtx_vertex_waves(").count(),
        1,
        "one helper"
    );
}
