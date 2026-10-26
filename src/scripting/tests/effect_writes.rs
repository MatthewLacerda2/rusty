//! `Component.Set` on a post effect's settings object (#1467): a table held from
//! `Get` writes only the leaves it edited, so a knob a tween moved since keeps the
//! tween's value, and a misspelt key inside an effect is an error.

use super::timers::{counted, rig, tick};

/// A rig whose entity carries a camera and the Add Component volume.
fn volume_rig(name: &str) -> (crate::scripting::manager::ScriptManager, u32) {
    let (mut m, id) = rig(name, &counted(""));
    let mut scene = m.cells.scene.borrow_mut();
    let vc = crate::scene::authoring::default_post_processing();
    scene.world.set_post_processing(id, Some(vc));
    scene.world.set_camera(id, Some(Default::default()));
    drop(scene);
    tick(&mut m, 1);
    (m, id)
}

#[test]
fn a_held_table_keeps_a_knob_a_tween_moved_since_get() {
    let (mut m, id) = volume_rig("effect_write_since_get");
    m.eval(&format!(
        "t = Component.Get({id}, 'PostProcessing') \
         Tween.To({id}, 'Graphics.bloom.intensity', 3, 2 / 60)"
    ))
    .unwrap();
    tick(&mut m, 3);
    m.eval(&format!(
        "t.bloom.active = false Component.Set({id}, 'PostProcessing', t)"
    ))
    .unwrap();
    let scene = m.cells.scene.borrow();
    let bloom = &scene.world.post_processing(id).unwrap().bloom;
    assert!(!bloom.active, "the edited leaf is written");
    assert_eq!(
        bloom.intensity, 3.0,
        "the tweened leaf keeps the tween's value"
    );
}

#[test]
fn an_unknown_key_inside_an_effect_is_an_error() {
    let (m, id) = volume_rig("effect_write_unknown");
    for table in [
        "{ bloom = { intensty = 2 } }",
        "{ exposure = { automatic = { compensaton = 1 } } }",
        "{ custom_effects = { nams = {} } }",
    ] {
        let err = m
            .eval(&format!("Component.Set({id}, 'PostProcessing', {table})"))
            .unwrap_err();
        assert!(err.contains("unknown field"), "{table}: {err}");
    }
    let scene = m.cells.scene.borrow();
    let vc = scene.world.post_processing(id).unwrap();
    assert_eq!(vc.bloom.intensity, 1.0, "nothing written");
}
