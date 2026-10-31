//! A material whose baked shader went stale against the engine draws it again once
//! the renderer re-bakes it from its recipe (#1638); a recipe-less module that no
//! longer composes keeps the standard look and is left as it is. Skips with no adapter.

use std::collections::BTreeMap;

use crate::render::test_gpu::headless_or_skip;
use crate::shadergen::recipe::{BlockSel, ParamValue, PassKind, ShaderRecipe};
use crate::shadergen::{bake_recipe, engine_shader_dir};

/// What a pre-#1616 bake looks like now: an old fingerprint over a module importing
/// a binding the engine no longer has.
const STRANDED: &str = "// rusty engine fingerprint: 0000000000000000\n\
    #import forward::bindings::{entity}\n\
    @fragment fn fs_main() -> @location(0) vec4<f32> { return entity.color_tint; }\n";

#[test]
fn gpu_a_stale_baked_shader_is_re_baked_and_drawn_a_bare_one_falls_back() {
    let Some(mut renderer) = headless_or_skip(32, 32) else {
        return;
    };
    let dir = crate::test_temp::dir().join(format!("rusty_1638_gpu_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dir_str = dir.to_str().unwrap().to_owned();
    renderer.surface_shaders.set_dirs(vec![dir_str.clone()]);
    let recipe = ShaderRecipe {
        pass: PassKind::Surface,
        name: "stale_tint".into(),
        blocks: vec![BlockSel {
            id: "tint".into(),
            params: BTreeMap::from([("color".into(), ParamValue::Vector(vec![0.0, 0.0, 1.0]))]),
        }],
    };
    bake_recipe(&recipe, engine_shader_dir(), &dir_str).expect("bakes");
    std::fs::write(dir.join("stale_tint.wgsl"), STRANDED).unwrap();
    std::fs::write(dir.join("bare.wgsl"), STRANDED).unwrap();

    let shaders = &mut renderer.surface_shaders;
    let id = shaders.pipeline_id(&renderer.device, Some("stale_tint"));
    assert_ne!(id, 0, "the re-baked module draws, not the standard shader");
    let rebaked = std::fs::read_to_string(dir.join("stale_tint.wgsl")).unwrap();
    assert_ne!(rebaked, STRANDED, "re-baked in place");

    let id = shaders.pipeline_id(&renderer.device, Some("bare"));
    assert_eq!(id, 0, "a recipe-less module that fails falls back");
    let bare = std::fs::read_to_string(dir.join("bare.wgsl")).unwrap();
    assert_eq!(bare, STRANDED, "and is left alone");
    std::fs::remove_dir_all(&dir).ok();
}
