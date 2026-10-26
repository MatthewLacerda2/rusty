//! Shader-param tweens (#661) without `from`: the start value is read through the
//! setter's getter (the override, else the material's, else the baked default),
//! a UI tween stops once its graphic switches shader, and a wrong-arity target
//! names the path it was given. Pins the survivors of #661's mutation run. A
//! volume's depth-of-field knob tweens the same way (#1445).

use super::timers::{counted, rig, tick};
use super::tween_shaders::{eval, Baked};

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
    m.cells.scene.borrow_mut().world.set_image(id, Some(image));
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
    assert!(m.cells.timers.borrow().is_empty(), "no shader, no tween");
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

#[test]
fn a_graphics_tween_racks_the_depth_of_field_focus() {
    let (mut m, id) = rig("tween_dof", &counted(""));
    let vc = crate::scene::authoring::default_post_processing();
    let camera = crate::components::CameraComponent::default();
    let mut scene = m.cells.scene.borrow_mut();
    scene.world.set_post_processing(id, Some(vc));
    scene.world.set_camera(id, Some(camera));
    drop(scene);
    tick(&mut m, 1);
    // A built-in knob (#1445), read from the volume: 10 m, Unity's default.
    m.eval(&format!(
        "Tween.To({id}, 'Graphics.depth_of_field.focus_distance', 2, 2 / 60)"
    ))
    .unwrap();
    // Started between ticks: the next tick is its own (the one-tick rule).
    tick(&mut m, 2);
    let get = "Graphics.GetDepthOfFieldFocusDistance()";
    assert_eq!(eval(&m, get), "6", "halfway from 10 to 2");
    tick(&mut m, 1);
    assert_eq!(eval(&m, get), "2");
    let err = m
        .eval(&format!(
            "Tween.To({id}, 'Graphics.depth_of_field.mode', 1, 1)"
        ))
        .unwrap_err();
    assert!(err.contains("tween property"), "{err}");
}

#[test]
fn every_effect_knob_tweens_by_its_path() {
    let (mut m, id) = rig("tween_knobs", &counted(""));
    let vc = crate::scene::authoring::default_post_processing();
    let mut scene = m.cells.scene.borrow_mut();
    scene.world.set_post_processing(id, Some(vc));
    scene.world.set_camera(id, Some(Default::default()));
    drop(scene);
    tick(&mut m, 1);
    // The lens strengths (#1439's lost tweens) and a nested knob (#1467).
    m.eval(&format!(
        "Tween.To({id}, 'Graphics.vignette.intensity', 0.9, 2 / 60) \
         Tween.To({id}, 'Graphics.exposure.automatic.compensation', 2, 2 / 60)"
    ))
    .unwrap();
    tick(&mut m, 3);
    let scene = m.cells.scene.borrow();
    let vc = scene.world.post_processing(id).unwrap();
    assert_eq!(vc.vignette.intensity, 0.9);
    assert_eq!(vc.exposure.automatic.compensation, 2.0);
}
