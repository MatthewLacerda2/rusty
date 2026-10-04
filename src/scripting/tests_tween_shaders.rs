//! Tweening runtime shader params (#661): `"Material.<p>"` eases the entity's own
//! override (#670), `"UI.<p>"` its ui shader's value, `"Graphics.<p>"` its volume's
//! post param (#671) — each through the setter's op, so the strict errors are the
//! setters' own.

use super::tests_timers::{counted, rig, tick};
use crate::scene::authoring::defaults;
use crate::shadergen::{bake_recipe, ShaderRecipe, DEFAULT_OUT_DIR, ENGINE_SHADER_DIR};

/// A shader baked from `pass` + `block` under a name unique to this test and run
/// (tests share a process under `cargo test`); removed on drop.
pub(super) struct Baked(pub(super) String);

impl Baked {
    pub(super) fn new(pass: &str, block: &str, tag: &str) -> Self {
        let name = format!("test_tween_{tag}_{}", std::process::id());
        let json = format!(r#"{{"pass":"{pass}","name":"{name}","blocks":[{{"id":"{block}"}}]}}"#);
        let recipe = ShaderRecipe::from_json(&json).unwrap();
        bake_recipe(&recipe, ENGINE_SHADER_DIR, DEFAULT_OUT_DIR).unwrap();
        Self(name)
    }
}

impl Drop for Baked {
    fn drop(&mut self) {
        for ext in ["wgsl", "params.json"] {
            let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{}.{ext}", self.0));
        }
    }
}

/// `expr` rounded to 3 decimals (f32 steps sum inexactly).
pub(super) fn eval(m: &super::manager::ScriptManager, expr: &str) -> String {
    m.eval(&format!("math.floor(({expr}) * 1000 + 0.5) / 1000"))
        .unwrap()
}

#[test]
fn a_material_tween_eases_the_entitys_override_not_the_shared_material() {
    let shader = Baked::new("surface", "hit_flash", "mat");
    let (mut m, id) = rig("tween_mat", &counted(""));
    tick(&mut m, 1);
    m.eval(&format!(
        "Material.SetShader({id}, '{}') \
         Tween.To({id}, 'Material.hit_flash.amount', 1, 4 / 60)",
        shader.0
    ))
    .unwrap();
    let get = format!("Material.GetShaderParam({id}, 'hit_flash.amount')");
    // Started between ticks: the next tick is its own (the one-tick rule).
    tick(&mut m, 3);
    assert_eq!(eval(&m, &get), "0.5");
    tick(&mut m, 2);
    assert_eq!(eval(&m, &get), "1");
    let s = m.scene.borrow();
    let key = &s.world.material(id).unwrap().material;
    assert!(
        s.materials[key].shader_params.is_empty(),
        "the asset is untouched"
    );
    assert_eq!(
        s.shader_overrides.of(id).unwrap()["hit_flash.amount"],
        [1.0]
    );
}

#[test]
fn a_ui_tween_runs_on_unscaled_time() {
    let shader = Baked::new("ui", "dissolve", "ui");
    let (mut m, id) = rig("tween_ui", &counted(""));
    let image = crate::components::ImageComponent::default();
    m.scene.borrow_mut().world.set_image(id, Some(image));
    tick(&mut m, 1);
    m.eval(&format!(
        "Time.SetTimeScale(0) UI.SetShader({id}, '{}') \
         Tween.To({id}, 'UI.dissolve.amount', 1, 2 / 60, {{ from = 0, unscaled = true }})",
        shader.0
    ))
    .unwrap();
    tick(&mut m, 2);
    let get = format!("UI.GetShaderParam({id}, 'dissolve.amount')");
    assert_eq!(eval(&m, &get), "0.5");
    tick(&mut m, 1);
    assert_eq!(eval(&m, &get), "1");
}

#[test]
fn a_graphics_tween_fades_the_volumes_post_param() {
    let effect = Baked::new("postfx", "damage_vignette", "post");
    let (mut m, id) = rig("tween_post", &counted(""));
    let mut vc = defaults::default_visual_correction();
    vc.custom_effects = vec![effect.0.clone()];
    m.scene
        .borrow_mut()
        .world
        .set_visual_correction(id, Some(vc));
    tick(&mut m, 1);
    m.eval(&format!(
        "Tween.To({id}, 'Graphics.damage_vignette.intensity', 0, 2 / 60, {{ from = 1 }})"
    ))
    .unwrap();
    let get = "Graphics.GetPostParam('damage_vignette.intensity')";
    assert_eq!(eval(&m, get), "1", "`from` is written at once");
    tick(&mut m, 2);
    assert_eq!(eval(&m, get), "0.5");
    tick(&mut m, 1);
    assert_eq!(eval(&m, get), "0");
    assert!(m.timers.borrow().is_empty(), "the finished tween retires");
}

#[test]
fn bad_shader_paths_are_errors_at_tween_to() {
    let shader = Baked::new("surface", "hit_flash", "err");
    let (m, id) = rig("tween_err", &counted(""));
    let err = |path: &str, target: &str| {
        m.eval(&format!("Tween.To({id}, '{path}', {target}, 1)"))
            .unwrap_err()
    };
    assert!(err("Material.hit_flash.amount", "1").contains("has no Material"));
    assert!(err("UI.dissolve.amount", "1").contains("has no Image, Shape or Text"));
    assert!(err("Graphics.vignette.strength", "1").contains("has no post-processing volume"));
    assert!(err("Shader.x.y", "1").contains("Material.<block.param>"));
    m.eval(&format!("Material.SetShader({id}, '{}')", shader.0))
        .unwrap();
    let typo = err("Material.hit_flash.amont", "1");
    assert!(
        typo.contains("hit_flash.amont") && typo.contains("hit_flash.amount"),
        "{typo}"
    );
    let arity = err("Material.hit_flash.color", "1");
    assert!(arity.contains("takes a table of 3 numbers"), "{arity}");
}
