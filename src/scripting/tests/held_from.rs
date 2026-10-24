//! An explicit `Tween.To` `from` (#1418): a clamped field starts at what its op
//! holds; a Transform field starts as given.

use super::timers::{counted, rig, tick};

/// `from = 500` on an alpha starts at 1, so one step of two reads 0.5.
#[test]
fn a_clamped_from_starts_at_the_held_value() {
    let body = "Scene.AddComponent(id, 'CanvasGroup') \
                Tween.To(id, 'CanvasGroup.alpha', 0, 2 / 60, { from = 500 })";
    let (mut m, id) = rig("tween_from_held", &counted(body));
    tick(&mut m, 2);
    let alpha = format!("Component.Get({id}, 'CanvasGroup').alpha");
    assert_eq!(m.eval(&alpha).unwrap(), "0.5");
}

/// A rotation's `from` is not read back through Euler angles (360 would read 0):
/// 360 → 180 is half a turn, so one step of two is at 270, read back as -90.
#[test]
fn a_rotation_from_starts_as_given() {
    let body = "Tween.To(id, 'Transform.rotation', {0, 180, 0}, 2 / 60, \
                { from = {0, 360, 0} })";
    let (mut m, id) = rig("tween_from_turn", &counted(body));
    tick(&mut m, 2);
    let yaw = format!("math.floor(({{Transform.GetRotation({id})}})[2] + 0.5)");
    assert_eq!(m.eval(&yaw).unwrap(), "-90");
}
