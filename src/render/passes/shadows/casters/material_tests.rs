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
    let _baked = Baked(name.clone());
    let path = bake_recipe(&recipe, engine_shader_dir(), DEFAULT_OUT_DIR).expect("bakes");
    let wgsl = std::fs::read_to_string(&path).expect("baked file");
    assert!(wgsl.contains(WAVES_NORMAL), "the helper's first line moved");
    std::fs::write(&path, wgsl.replacen(WAVES_NORMAL, READS_MATERIAL, 1)).unwrap();
    let built = crate::render::test_gpu::headless_or_skip(16, 16).map(|mut r| {
        let id = r.surface_shaders.pipeline_id(&r.device, Some(&name));
        (id != STANDARD, r.surface_shaders.cut(id).is_some())
    });
    assert_ne!(built, Some((false, false)), "the variant was refused");
    assert_ne!(
        built,
        Some((true, false)),
        "its depth pipelines were refused"
    );
}

/// The variant's baked files, removed when the test ends, whether it passes or not.
struct Baked(String);

impl Drop for Baked {
    fn drop(&mut self) {
        for ext in ["wgsl", "params.json", "recipe.json"] {
            let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{}.{ext}", self.0));
        }
    }
}

/// Two Cutout materials that share an albedo map but differ in tint clip their
/// shadows alike: the standard cutout reads its test per caster and never the
/// record, so its clip carries none and both stay one caster batch.
#[test]
fn gpu_cutouts_differing_only_in_tint_share_one_caster_batch() {
    use super::{batch_casters, CasterData, CasterKey};
    use crate::components::RenderMode;
    use crate::render::gpu::pipelines::winding::Facing;
    use crate::scene::authoring::{create_entity, material, Primitive};
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let map = crate::test_temp::dir().join(format!("rusty_tint_leaf_{}.png", std::process::id()));
    image::RgbaImage::new(1, 1)
        .save(&map)
        .expect("albedo written");
    let mut scene = crate::scene::Scene::new();
    let leaves = [[0.2, 0.8, 0.2], [0.6, 0.9, 0.1]].map(|rgb| {
        let id = create_entity(&mut scene, "Leaf", Some(Primitive::Plane));
        let key = material::ensure_material_key(&mut scene, id).unwrap();
        let asset = scene.materials.get_mut(&key).unwrap();
        asset.base_color_map = Some(map.to_string_lossy().into_owned().into());
        (asset.base_color, asset.render_mode) = (rgb, RenderMode::Cutout);
        id
    });
    let clips = r.shadow_clips(&scene);
    let _ = std::fs::remove_file(&map);
    let casters = leaves
        .map(|id| {
            let key = CasterKey {
                facing: Facing::Front,
                clip: Some(clips[&(id, 0)].clip),
                mesh: crate::render::MeshId("Plane".into()),
                indices: (0..6).into(),
            };
            (key, CasterData::new(glam::Mat4::IDENTITY, 0))
        })
        .into();
    let (_, batches) = batch_casters(casters, true);
    assert_eq!(batches.len(), 1, "the tint split the shadow draw");
}
