use super::timers::{counted, rig, tick};

#[test]
fn a_clamped_from_starts_at_the_held_value() {
    let body = "Scene.AddComponent(id, 'CanvasGroup') \
                Tween.To(id, 'CanvasGroup.alpha', 0, 2 / 60, { from = 500 })";
    let (mut m, id) = rig("tween_from_held", &counted(body));
    tick(&mut m, 2);
    let alpha = format!("Component.Get({id}, 'CanvasGroup').alpha");
    assert_eq!(m.eval(&alpha).unwrap(), "0.5");
}

#[test]
fn a_rotation_from_starts_as_given() {
    let body = "Tween.To(id, 'Transform.rotation', {0, 180, 0}, 2 / 60, \
                { from = {0, 360, 0} })";
    let (mut m, id) = rig("tween_from_turn", &counted(body));
    tick(&mut m, 2);
    let yaw = format!("math.floor(({{Transform.GetRotation({id})}})[2] + 0.5)");
    assert_eq!(m.eval(&yaw).unwrap(), "-90");
}
