//! Shader-param tweens (#661) without `from`: the start value is read through the
//! setter's getter (the override, else the material's, else the baked default),
//! a UI tween stops once its graphic switches shader, and a wrong-arity target
//! names the path it was given. Pins the survivors of #661's mutation run.

use super::tests_timers::{counted, rig, tick};
use super::tests_tween_shaders::{eval, Baked};

#[test]
fn a_material_tween_starts_from_the_entitys_current_override() {
    let shader = Baked::new("surface", "hit_flash", "read_mat");
    let (mut m, id) = rig("tween_read_mat", &counted(""));
    tick(&mut m, 1);
    m.eval(&format!(
        "Material.SetShader({id}, '{}') \
         Material.SetShaderParam({id}, 'hit_flash.amount', 0.5) \
         Tween.To({id}, 'Material.hit_flash.amount', 1, 2 / 60)",
        shader.0
    ))
    .unwrap();
    // Started between ticks: the next tick is its own (the one-tick rule).
    tick(&mut m, 2);
    let get = format!("Material.GetShaderParam({id}, 'hit_flash.amount')");
    assert_eq!(eval(&m, &get), "0.75", "halfway from 0.5, not from 0");
}

#[test]
fn a_ui_tween_reads_its_start_and_ends_when_the_shader_goes() {
    let shader = Baked::new("ui", "dissolve", "read_ui");
    let (mut m, id) = rig("tween_read_ui", &counted(""));
    let image = crate::components::ImageComponent::default();
    m.scene.borrow_mut().world.set_image(id, Some(image));
    tick(&mut m, 1);
    m.eval(&format!(
        "UI.SetShader({id}, '{}') \
         UI.SetShaderParam({id}, 'dissolve.amount', 0.5) \
         Tween.To({id}, 'UI.dissolve.amount', 1, 4 / 60)",
        shader.0
    ))
    .unwrap();
    tick(&mut m, 2);
    let get = format!("UI.GetShaderParam({id}, 'dissolve.amount')");
    assert_eq!(eval(&m, &get), "0.625", "a quarter of the way from 0.5");
    m.eval(&format!("UI.SetShader({id}, nil)")).unwrap();
    tick(&mut m, 1);
    assert!(m.timers.borrow().is_empty(), "no shader, no tween");
}

#[test]
fn a_wrong_arity_target_names_the_path() {
    let shader = Baked::new("surface", "hit_flash", "read_arity");
    let (m, id) = rig("tween_read_arity", &counted(""));
    m.eval(&format!("Material.SetShader({id}, '{}')", shader.0))
        .unwrap();
    for path in ["Material.hit_flash.color", "Transform.position"] {
        let err = m
            .eval(&format!("Tween.To({id}, '{path}', 1, 1)"))
            .unwrap_err();
        assert!(err.contains(&format!("`{path}` takes")), "{err}");
    }
}
