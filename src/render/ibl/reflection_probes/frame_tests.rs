//! Choosing the frame's resident probes (#1238): ranked by box, ties by scene index,
//! cubes that don't fit passed over before the budget.

use std::cell::RefCell;
use std::panic::AssertUnwindSafe;
use std::rc::Rc;

use glam::Vec3;

use super::super::gpu_tests::{cube_file, first, probes_at, renderer};
use super::ranked;
use crate::render::ibl::reflection_probes::MAX_REFLECTION_PROBES;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

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
    assert_eq!(probes.uniforms()[first(p)].center, [0.0, -90.0, 0.0]);
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
        probes.uniforms()[first(probes.probe_for(Vec3::ZERO))].center
    };
    let left = centre_from(&mut renderer, -0.5);
    assert_eq!(left, centre_from(&mut renderer, 0.5));
    assert_eq!(left, [-1.0, 0.0, 0.0], "the lower scene index");
}

/// A cube whose size is not a power of two is passed over: it neither takes a budget
/// slot nor knocks out the other probes, it is never uploaded, and it warns once per
/// load: again only after a re-bake reads it again.
#[test]
fn gpu_a_larger_cube_is_passed_over_and_never_uploaded() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let (big, small) = (
        cube_file("odd_48.ktx2", 48, 5),
        cube_file("small_4.ktx2", 4, 5),
    );
    let scene = probes_at(&[(-2.0, &big), (2.0, &small)]);
    crate::logging::init();
    let console = RefCell::new(ConsoleLogs::new());
    let warnings = |renderer: &mut crate::render::Renderer| {
        renderer.update_reflection_probes(&scene, Vec3::ZERO);
        let logged = crate::logging::forward(&console);
        logged.iter().filter(|m| m.contains(big.as_str())).count()
    };
    assert_eq!(warnings(&mut renderer), 1, "the first frame warns");
    assert_eq!(warnings(&mut renderer), 0, "the next frame does not");
    let probes = &renderer.reflection_probes;
    assert_eq!(probes.probe_for(Vec3::new(-2.0, 0.0, 0.0)), 0);
    assert_ne!(probes.probe_for(Vec3::new(2.0, 0.0, 0.0)), 0);
    let cached = renderer.cubemaps.get(&big);
    assert!(matches!(cached, Some(None)), "cached as absent, no texture");
    renderer.invalidate_textures(std::slice::from_ref(&big));
    assert_eq!(
        warnings(&mut renderer),
        1,
        "a re-bake reads it and warns again"
    );
}

/// `update` takes only admitted cubes: one whose size is not a power of two would give
/// the array a mip count wgpu rejects, so a debug build stops at the contract.
#[test]
#[cfg(debug_assertions)]
fn gpu_update_refuses_a_cube_that_does_not_fit() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let path = cube_file("unadmitted_48.ktx2", 48, 5);
    let cube = Rc::new(renderer.load_cubemap_mips(&path, |_| true).unwrap());
    let scene = probes_at(&[(0.0, &path)]);
    let wanted = [(&scene.reflection_probes.probes[0], cube)];
    let r = &mut renderer;
    let update = AssertUnwindSafe(|| {
        r.reflection_probes
            .update(&r.device, &r.queue, &wanted, u64::MAX)
    });
    let err = std::panic::catch_unwind(update).expect_err("the assert trips");
    let message = (err.downcast_ref::<&str>().map(|m| m.to_string()))
        .or_else(|| err.downcast_ref::<String>().cloned())
        .unwrap_or_default();
    assert!(
        message.contains("every wanted cube was admitted"),
        "{message}"
    );
}
