//! Choosing the frame's resident probes (#1238): ranked by box, ties by scene index,
//! cubes that don't fit passed over before the budget.

use glam::Vec3;

use super::super::gpu_tests::{cube_file, probes_at, renderer};
use super::ranked;
use crate::render::ibl::reflection_probes::MAX_REFLECTION_PROBES;
use crate::scene::Scene;

/// A probe whose box holds the camera ranks first, however far its capture point.
#[test]
fn a_box_holding_the_camera_ranks_first() {
    let mut scene = probes_at(&[(3.0, "near"), (50.0, "room")]);
    let set = &mut scene.reflection_probes;
    set.set_box(1, Vec3::splat(-60.0), Vec3::splat(60.0));
    assert_eq!(ranked(&set.probes, Vec3::ZERO), vec![1, 0]);
}

/// A level of more probes than the budget: the camera's own room has the farthest
/// capture point and still keeps its probe.
#[test]
fn gpu_the_camera_room_keeps_its_probe_past_the_budget() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let cube = cube_file("budget.ktx2", 4, 70);
    let mut scene = Scene::new();
    let set = &mut scene.reflection_probes;
    for k in 0..MAX_REFLECTION_PROBES + 8 {
        let i = set.add(Vec3::new(2.0 + k as f32, 5.0, 0.0), Vec3::splat(0.4));
        set.set_cubemap(i, cube.clone());
    }
    let room = set.add(Vec3::new(0.0, -90.0, 0.0), Vec3::ONE);
    set.set_box(room, Vec3::splat(-100.0), Vec3::splat(100.0));
    set.set_cubemap(room, cube);
    renderer.update_reflection_probes(&scene, Vec3::ZERO);
    let probes = &renderer.reflection_probes;
    let p = probes.probe_for(Vec3::new(0.0, -50.0, 0.0));
    assert_ne!(p, 0, "the room's probe is resident");
    assert_eq!(probes.uniforms()[p as usize - 1].center, [0.0, -90.0, 0.0]);
}

/// Two symmetric probes covering one object give it the same probe from either side
/// of the midline.
#[test]
fn gpu_symmetric_probes_do_not_snap_with_the_camera() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let (a, b) = (
        cube_file("sym_a.ktx2", 4, 30),
        cube_file("sym_b.ktx2", 4, 60),
    );
    let mut scene = probes_at(&[(-1.0, &a), (1.0, &b)]);
    for i in 0..2 {
        let set = &mut scene.reflection_probes;
        set.set_box(i, Vec3::splat(-3.0), Vec3::splat(3.0));
    }
    let centre_from = |renderer: &mut crate::render::Renderer, x: f32| {
        renderer.update_reflection_probes(&scene, Vec3::new(x, 0.0, 0.0));
        let probes = &renderer.reflection_probes;
        let p = probes.probe_for(Vec3::ZERO) as usize;
        probes.uniforms()[p - 1].center
    };
    let left = centre_from(&mut renderer, -0.5);
    assert_eq!(left, centre_from(&mut renderer, 0.5));
    assert_eq!(left, [-1.0, 0.0, 0.0], "the lower scene index");
}

/// A larger hand-set cube is passed over: it neither takes a budget slot nor knocks
/// out the baked-size probes, and a re-bake of it warns again.
#[test]
fn gpu_a_larger_cube_is_passed_over_and_warns_after_a_rebake() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let (big, small) = (
        cube_file("big_256.ktx2", 256, 5),
        cube_file("small_4.ktx2", 4, 5),
    );
    let scene = probes_at(&[(-2.0, &big), (2.0, &small)]);
    renderer.update_reflection_probes(&scene, Vec3::ZERO);
    let probes = &mut renderer.reflection_probes;
    assert_eq!(probes.probe_for(Vec3::new(-2.0, 0.0, 0.0)), 0);
    assert_ne!(probes.probe_for(Vec3::new(2.0, 0.0, 0.0)), 0);
    assert_eq!(probes.warned, vec![big.clone()]);
    probes.forget(|p| p == big);
    assert!(probes.warned.is_empty(), "the next frame warns again");
}
