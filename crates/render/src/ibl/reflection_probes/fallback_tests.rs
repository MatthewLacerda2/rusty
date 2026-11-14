//! The fallback probe among the frame's residents (#1697): ranked first and slot 0,
//! and never at the cost of the camera's own room's probe.

use glam::Vec3;

use super::super::gpu_tests::{cube_file, first, probes_at, renderer};
use super::ranked;

/// The fallback leads, then the probes by box distance, ties to the lower index.
#[test]
fn the_fallback_ranks_first_then_the_nearest() {
    let mut scene = probes_at(&[(9.0, "a"), (-3.0, "b"), (40.0, "fallback"), (3.0, "c")]);
    let set = &mut scene.reflection_probes;
    set.set_fallback(Some(2));
    assert_eq!(ranked(&set.probes, Vec3::ZERO), vec![2, 1, 3, 0]);
    set.set_fallback(None);
    assert_eq!(ranked(&set.probes, Vec3::ZERO), vec![1, 3, 0, 2]);
}

/// A far fallback whose cube alone fills the budget does not evict the camera's own
/// room's probe: both are taken whatever the budget, the fallback in slot 0.
#[test]
fn gpu_a_big_fallback_keeps_the_nearest_probe_resident() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let (big, near, far) = (
        cube_file("fallback_256.ktx2", 256, 40),
        cube_file("fallback_near_4.ktx2", 4, 80),
        cube_file("fallback_far_4.ktx2", 4, 120),
    );
    let mut scene = probes_at(&[(30.0, &far), (0.0, &near), (60.0, &big)]);
    scene.reflection_probes.set_fallback(Some(2));
    scene.lighting_settings.reflections.budget_mb = 1;
    renderer.update_reflection_probes(&scene, Vec3::ZERO);
    let probes = &renderer.reflection_probes;
    let uniforms = probes.uniforms();
    assert_eq!(
        uniforms[0].center,
        [60.0, 0.0, 0.0],
        "the fallback is slot 0"
    );
    assert_eq!(uniforms[0].params[2], 1.0, "flagged as the fallback");
    let room = first(probes.probe_for(Vec3::ZERO));
    assert_eq!(uniforms[room].center, [0.0, 0.0, 0.0], "the camera's room");
    assert_eq!(
        probes.probe_for(Vec3::new(30.0, 0.0, 0.0)),
        0,
        "the farther probe is past the budget"
    );
}
