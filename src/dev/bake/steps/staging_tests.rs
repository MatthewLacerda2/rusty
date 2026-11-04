//! A bake plans its probe sets into copies and applies them only when it commits
//! (#1737): refused or without an adapter, the scene keeps its old probes and their
//! lighting. The GPU paths are in `staging_gpu_tests.rs`.

use glam::Vec3;

use super::super::lighting::{plan_probes, start_lighting, LightingBakeParams};
use super::LightingBake;
use crate::components::{ColliderComponent, ColliderShape};
use crate::scene::lighting::probe::ProbeVolume;
use crate::scene::{ReflectionProbeSet, Scene};

/// A walled room the placement plans both sets in, lit by an earlier bake: two
/// placed light probes with SH, one placed reflection probe with a cube.
pub(super) fn lit_room() -> Scene {
    let mut scene = Scene::new();
    let id = scene.add_entity("Room".to_string());
    scene.world.set_static(id, true);
    let size = Vec3::new(20.0, 6.0, 20.0);
    let collider = ColliderComponent {
        shape: ColliderShape::Box { size },
        ..crate::scene::authoring::default_collider()
    };
    scene.world.set_collider(id, Some(collider));
    for x in [-2.0, 2.0] {
        let i = scene.probes.add_probe(Vec3::new(x, 4.0, 0.0));
        scene.probes.probes[i].sh.coeffs[0] = [0.5, 0.4, 0.3];
    }
    let r = scene
        .reflection_probes
        .add(Vec3::new(0.0, 4.6, 0.0), Vec3::splat(3.0));
    scene.reflection_probes.set_cubemap(r, "old.ktx2".into());
    scene.lighting_settings.reflections.resolution = 16;
    scene
}

pub(super) fn params() -> LightingBakeParams {
    let mut params = LightingBakeParams {
        probe_spacing: 10.0,
        ..Default::default()
    };
    params.reflection.region = 40.0;
    params
}

/// The scene's probes are the ones `lit_room` gave it, lighting and all.
pub(super) fn assert_old_lighting(scene: &Scene, old: &(ProbeVolume, ReflectionProbeSet)) {
    let light = |v: &ProbeVolume| {
        v.probes
            .iter()
            .map(|p| (p.position, p.sh))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        light(&scene.probes),
        light(&old.0),
        "old light probes and SH"
    );
    assert_eq!(
        scene.reflection_probes.probes, old.1.probes,
        "old reflection probes"
    );
}

pub(super) fn old(scene: &Scene) -> (ProbeVolume, ReflectionProbeSet) {
    (scene.probes.clone(), scene.reflection_probes.clone())
}

pub(super) fn saved_path(name: &str) -> String {
    let dir = crate::test_temp::dir().join(format!("rusty_1737_{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("level.scene").to_string_lossy().into_owned()
}

#[test]
fn a_bake_refused_for_no_scene_path_keeps_the_old_lighting() {
    let scene = lit_room();
    let was = old(&scene);
    let refused = start_lighting(&scene, None, None, params());
    assert!(refused.is_err_and(|e| e.contains("save the scene")));
    assert_old_lighting(&scene, &was);
}

#[test]
fn without_an_adapter_nothing_is_applied() {
    let mut scene = lit_room();
    let was = old(&scene);
    let (planned, report) = plan_probes(&scene, None, params());
    assert!(report.auto_placed_light && report.auto_placed_reflection);
    let path = saved_path("no_adapter");
    let mut job = LightingBake::on(None, &scene, Some(&path), planned, report).unwrap();
    assert!(
        job.step(&mut scene, &mut |_| {}).unwrap(),
        "nothing to step"
    );
    job.commit(&mut scene).unwrap();
    assert!(!job.report().ran_light_bake && !job.report().ran_reflection_bake);
    assert_old_lighting(&scene, &was);
}

/// With the static geometry deleted and no manual probe, the plan is empty and its
/// commit removes the old probes: "nothing to bake" is not "no adapter".
#[test]
fn an_empty_plan_removes_every_probe() {
    let mut scene = lit_room();
    for id in scene.world.ids_with_collider() {
        scene.world.set_collider(id, None);
    }
    let (planned, report) = plan_probes(&scene, None, params());
    assert!(planned.light.is_empty() && planned.reflections.is_empty());
    let mut job = LightingBake::on(None, &scene, None, planned, report).unwrap();
    assert!(job.step(&mut scene, &mut |_| {}).unwrap());
    job.commit(&mut scene).unwrap();
    assert!(scene.probes.is_empty() && scene.reflection_probes.is_empty());
}
