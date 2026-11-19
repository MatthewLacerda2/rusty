//! The pick a draw's instance carries (#2324): as the camera crosses from one probe
//! to the next and the resident set changes, `probe_for` reads the prepared set and
//! still equals a blend of the residents built afresh from the scene.

use glam::Vec3;

use super::super::gpu_tests::{cube_file, probes_at, renderer};
use super::super::pack_blend;
use crate::Renderer;
use rusty_scene::{ReflectionProbeSet, Scene};

/// The scene's resident probes, cloned from the scene in resident order: an
/// unprepared set nothing was kept for.
fn fresh(r: &Renderer, scene: &Scene) -> ReflectionProbeSet {
    let mut held: Vec<_> = (scene.reflection_probes.probes.iter())
        .filter_map(|p| Some((r.reflection_probes.resident(p)?, p.clone())))
        .collect();
    held.sort_by_key(|&(i, _)| i);
    ReflectionProbeSet {
        probes: held.into_iter().map(|(_, p)| p).collect(),
    }
}

#[test]
fn gpu_a_camera_crossing_probes_picks_as_a_fresh_blend() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let cube = cube_file("pick_4.ktx2", 4, 50);
    let mut scene = probes_at(&[(0.0, &cube), (2.5, &cube), (5.0, &cube), (7.5, &cube)]);
    let turned = scene.reflection_probes.edit(2, |p| {
        p.rotation = glam::Quat::from_rotation_y(0.6);
        p.blend_distance = 1.5;
    });
    assert!(turned);
    // A zero budget keeps only the nearest probe: each step moves the resident set.
    scene.lighting_settings.reflections.budget_mb = 0;
    let mut seen = Vec::new();
    for step in 0..=30 {
        let camera = Vec3::new(step as f32 * 0.25, 0.0, 0.0);
        renderer.update_reflection_probes(&scene, camera);
        let set = fresh(&renderer, &scene);
        seen.push(set.probes.first().map(|p| p.position.x));
        for k in -40..=120 {
            let point = Vec3::new(k as f32 * 0.07, 0.3, -0.2);
            let picked = renderer.reflection_probes.probe_for(point);
            assert_eq!(
                picked,
                pack_blend(set.blend(point)),
                "camera {camera}, at {point}"
            );
        }
    }
    seen.dedup();
    assert!(
        seen.len() >= 4,
        "the resident set changed on the way: {seen:?}"
    );
}
