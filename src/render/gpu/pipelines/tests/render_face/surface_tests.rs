//! An authored surface shader on a two-sided face (#1615): its colour blocks read
//! the normal the face is shaded with, turned toward the viewer when it is seen
//! from behind, so a Fresnel rim on a `Both` card seen head-on from behind stays
//! dark, as from the front, instead of glowing over the whole card. Its facing
//! pipelines are built with the slot in a warmed (windowed) renderer. Skips with
//! no adapter.

use std::collections::BTreeMap;

use super::render_face_tests::{asset, card, frame, mean, RES};
use crate::components::RenderFace;
use crate::render::Renderer;
use crate::shadergen::recipe::{BlockSel, ParamValue, PassKind, ShaderRecipe};
use crate::shadergen::{bake_recipe, engine_shader_dir};

/// Bake `name` into `dir`: one white `fresnel_rim` of `power` (0 glows everywhere).
fn bake_rim(dir: &str, name: &str, power: f32) {
    let params = BTreeMap::from([
        ("color".to_string(), ParamValue::Vector(vec![4.0; 3])),
        ("power".to_string(), ParamValue::Scalar(power)),
    ]);
    let blocks = vec![BlockSel {
        id: "fresnel_rim".into(),
        params,
    }];
    let recipe = ShaderRecipe {
        pass: PassKind::Surface,
        name: name.into(),
        blocks,
    };
    bake_recipe(&recipe, engine_shader_dir(), dir).expect("bake succeeds");
}

/// The `Both` card seen head-on from behind, shaded by `shader` (`None`: standard).
fn behind(r: &mut Renderer, shader: Option<&str>) -> f32 {
    let (mut scene, cam) = card(RenderFace::Both, false, true);
    let id = scene.world.find_by_name("Card").expect("the card");
    asset(&mut scene, id).shader = shader.map(str::to_owned);
    mean(&frame(r, &scene, &cam))
}

#[test]
fn gpu_a_rim_on_a_two_sided_card_seen_from_behind_stays_dark() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let dir = crate::test_temp::dir().join(format!("rusty_face_rim_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.to_string_lossy().into_owned();
    bake_rim(&dir, "face_rim", 4.0);
    bake_rim(&dir, "face_glow", 0.0);
    r.surface_shaders.set_dirs(vec![dir]);
    r.warm_pipelines(false);
    let (plain, rim, glow) = (
        behind(&mut r, None),
        behind(&mut r, Some("face_rim")),
        behind(&mut r, Some("face_glow")),
    );
    eprintln!("[render face rim] from behind: plain {plain:.1}, rim {rim:.1}, glow {glow:.1}");
    assert!(glow > plain + 30.0, "control: the block draws");
    assert!(
        (rim - plain).abs() < 4.0,
        "head-on, the seen face has no rim"
    );
    let built = r.surface_shaders.lazy_pipelines().collect::<Vec<_>>();
    assert!(!built.is_empty() && built.iter().all(|p| p.is_built()));
}
