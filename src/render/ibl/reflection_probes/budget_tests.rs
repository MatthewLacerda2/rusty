//! The byte budget (#1584): it bounds the resident set, the array and the decoded
//! cubes the renderer keeps, and one big cube does not empty the set.

use glam::Vec3;

use super::super::gpu_tests::{cube_file, probes_at, renderer};
use super::array_bytes;
use crate::render::Renderer;
use crate::scene::Scene;

/// The packed blend of the object at `x`, from `r`'s resident probes.
fn probe_at(r: &Renderer, x: f32) -> u32 {
    r.reflection_probes.probe_for(Vec3::new(x, 0.0, 0.0))
}

/// The resident set stops at the scene's byte budget, counted at the array's face;
/// the nearest box is kept whatever its size.
#[test]
fn gpu_the_budget_keeps_the_nearest_probes() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let cube = cube_file("budget_4.ktx2", 4, 70);
    let mut scene = probes_at(&[(2.0, &cube), (6.0, "x"), (10.0, &cube)]);
    scene
        .reflection_probes
        .set_cubemap(1, cube_file("budget_b.ktx2", 4, 20));
    scene.lighting_settings.reflections.budget_mb = 0;
    renderer.update_reflection_probes(&scene, Vec3::ZERO);
    assert_ne!(
        probe_at(&renderer, 2.0),
        0,
        "the nearest is kept past a zero budget"
    );
    assert_eq!(probe_at(&renderer, 6.0), 0, "the next is past the budget");
    assert_eq!((array_bytes(4, 1), array_bytes(512, 1)), (512, 8_388_608));
}

/// A cube too big for the budget is passed over; the farther ones that fit are
/// still taken.
#[test]
fn gpu_one_big_cube_does_not_empty_the_resident_set() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let small = cube_file("pass_small.ktx2", 64, 30);
    let big = cube_file("pass_big.ktx2", 512, 60);
    let far = cube_file("pass_far.ktx2", 64, 90);
    let mut scene = probes_at(&[(0.0, &small), (4.0, &big), (8.0, &far)]);
    scene.lighting_settings.reflections.budget_mb = 1;
    renderer.update_reflection_probes(&scene, Vec3::ZERO);
    assert_eq!(probe_at(&renderer, 4.0), 0, "the big cube is passed over");
    assert_ne!(probe_at(&renderer, 8.0), 0, "the far one still fits");
    assert_eq!(
        renderer.reflection_probes.face(),
        64,
        "the face did not grow"
    );
    assert!(
        !renderer.cubemaps.contains_key(&big),
        "and the big one is evicted"
    );
}

/// A level of more probes than the budget, walked end to end: the array and the
/// decoded cubes never pass the budget, and the probe underfoot stays resident.
#[test]
fn gpu_a_walk_through_the_level_stays_within_the_budget() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut scene = Scene::new();
    for k in 0..40 {
        let set = &mut scene.reflection_probes;
        let i = set.add(Vec3::new(4.0 * k as f32, 0.0, 0.0), Vec3::ONE);
        set.set_cubemap(i, cube_file(&format!("walk_{k}.ktx2"), 64, k as u8));
    }
    scene.lighting_settings.reflections.budget_mb = 1;
    let budget = 1 << 20;
    for step in 0..=80 {
        let x = 2.0 * step as f32;
        renderer.update_reflection_probes(&scene, Vec3::new(x, 0.0, 0.0));
        let probes = &renderer.reflection_probes;
        let held = renderer.cubemaps.values().flatten().count() as u64;
        assert!(
            held * array_bytes(64, 1) <= budget,
            "{held} cubes held at {x}"
        );
        assert!(probes.cache.held_bytes() <= budget);
        assert!(array_bytes(probes.face(), probes.cubes) <= budget, "at {x}");
        let underfoot = (x / 4.0).round().min(39.0) * 4.0;
        assert_ne!(probe_at(&renderer, underfoot), 0, "at {x}");
    }
}
