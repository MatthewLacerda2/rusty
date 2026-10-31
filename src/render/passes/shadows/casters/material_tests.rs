//! A clipped caster's draw reads its material (#1616), as Unity's ShadowCaster pass
//! does: the shadow layout carries the forward pass's per-draw uniform and material
//! array, so a vertex block reading `material()` in `vs_shadow` builds.

use crate::render::gpu::pipelines::surface::STANDARD;
use crate::shadergen::recipe::{BlockSel, PassKind, ShaderRecipe};
use crate::shadergen::{bake_recipe, engine_shader_dir, DEFAULT_OUT_DIR};

/// What the catalog's `vertex_waves` opens with, and the same line scaled by the
/// record's tint alpha: a vertex block that reads the material.
const WAVES_NORMAL: &str = "let n = normalize(v.normal);";
const READS_MATERIAL: &str = "let n = normalize(v.normal) * material().color_tint.a;";

/// The variant's forward pipelines and its depth ones (`vs_shadow` through the
/// shadow layout) all build: without the material array in the shadow layout wgpu
/// refuses the shadow pipeline and the whole variant falls back to the standard
/// shader.
#[test]
fn gpu_a_vertex_block_reading_the_material_builds_its_shadow_pipelines() {
    let name = format!("test_material_vertex_{}", std::process::id());
    let recipe = ShaderRecipe {
        pass: PassKind::Surface,
        name: name.clone(),
        blocks: vec![BlockSel {
            id: "vertex_waves".into(),
            params: Default::default(),
        }],
    };
    let path = bake_recipe(&recipe, engine_shader_dir(), DEFAULT_OUT_DIR).expect("bakes");
    let wgsl = std::fs::read_to_string(&path).expect("baked file");
    assert!(wgsl.contains(WAVES_NORMAL), "the helper's first line moved");
    std::fs::write(&path, wgsl.replacen(WAVES_NORMAL, READS_MATERIAL, 1)).unwrap();
    let built = crate::render::test_gpu::headless_or_skip(16, 16).map(|mut r| {
        let id = r.surface_shaders.pipeline_id(&r.device, Some(&name));
        (id != STANDARD, r.surface_shaders.cut(id).is_some())
    });
    for ext in ["wgsl", "params.json"] {
        let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{name}.{ext}"));
    }
    assert_ne!(built, Some((false, false)), "the variant was refused");
    assert_ne!(
        built,
        Some((true, false)),
        "its depth pipelines were refused"
    );
}
