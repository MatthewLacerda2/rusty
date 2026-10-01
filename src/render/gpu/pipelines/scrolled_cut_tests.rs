//! A surface variant whose cut follows a UV-stage block (#648) builds its depth
//! pipelines. Skips with no adapter.

use crate::shadergen::recipe::{BlockSel, PassKind, ShaderRecipe};
use crate::shadergen::{bake_recipe, DEFAULT_OUT_DIR, ENGINE_SHADER_DIR};

/// A scrolled cut (#648) builds its depth pipelines: its shadow cut reads
/// `camera.time`, which the shadow pass binds — wgpu would refuse it otherwise and
/// the variant would quietly fall back to the standard shader.
#[test]
fn gpu_a_scrolled_dissolve_builds_its_cut_pipelines() {
    let name = format!("test_scrolled_cut_{}", std::process::id());
    let blocks = ["uv_scroll", "dissolve"].map(|id| BlockSel {
        id: id.into(),
        params: Default::default(),
    });
    let recipe = ShaderRecipe {
        pass: PassKind::Surface,
        name: name.clone(),
        blocks: blocks.to_vec(),
    };
    bake_recipe(&recipe, ENGINE_SHADER_DIR, DEFAULT_OUT_DIR).expect("bake succeeds");
    let r = crate::render::test_gpu::headless_or_skip(16, 16);
    let built = r.map(|mut r| {
        let id = r.surface_shaders.pipeline_id(&r.device, Some(&name));
        r.surface_shaders.cut(id).is_some()
    });
    for ext in ["wgsl", "params.json"] {
        let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{name}.{ext}"));
    }
    assert_ne!(
        built,
        Some(false),
        "the variant and its cut pipelines build"
    );
}
