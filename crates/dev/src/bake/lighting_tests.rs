use glam::Vec3;

use super::{bake_lighting, LightingBakeParams};
use crate::bake::progress::BakeLog;
use rusty_components::{ColliderComponent, ColliderShape};
use rusty_scene::Scene;

fn room_scene() -> Scene {
    let mut scene = Scene::new();
    let id = scene.add_entity("Floor".to_string());
    scene.world.set_static(id, true);
    scene.world.transform_mut(id).unwrap().position = Vec3::ZERO;
    scene.world.set_collider(
        id,
        Some(ColliderComponent {
            shape: ColliderShape::Box {
                size: Vec3::new(20.0, 6.0, 20.0),
            },
            ..rusty_edit::authoring::default_collider()
        }),
    );
    scene
}

#[test]
fn gpu_auto_places_both_sets_on_empty_scene() {
    let dir = rusty_test_support::temp::dir().join("rusty_lighting_bake_test");
    let _ = std::fs::create_dir_all(&dir);
    let scene_path = dir.join("room.scene");
    let scene_path = scene_path.to_str().unwrap();

    let mut scene = room_scene();
    let mut coarse = LightingBakeParams {
        probe_spacing: 10.0,
        ..Default::default()
    };
    coarse.reflection.region = 40.0;
    scene.lighting_settings.reflections.resolution = 16;
    let report = bake_lighting(
        &mut scene,
        Some(scene_path),
        None,
        coarse,
        &mut |_| {},
        &mut BakeLog::off(),
    )
    .expect("placement + adapter-free bake never errors with a scene path");

    assert!(report.auto_placed_light, "light probes auto-placed");
    assert!(
        report.auto_placed_reflection,
        "reflection probes auto-placed"
    );
    assert!(report.light_probes > 0, "grid produced light probes");
    assert!(
        report.reflection_probes > 0,
        "subdivision produced reflection probes"
    );
    if !report.ran_light_bake {
        return;
    }
    assert_eq!(scene.probes.probes().len(), report.light_probes);
    assert_eq!(scene.reflection_probes.len(), report.reflection_probes);
}

#[test]
fn gpu_manual_probes_survive_a_replacement() {
    let dir = rusty_test_support::temp::dir().join("rusty_lighting_bake_test");
    let _ = std::fs::create_dir_all(&dir);
    let scene_path = dir.join("manual.scene");
    let scene_path = scene_path.to_str().unwrap();

    let (mut scene, manual) = with_manual_probes();
    let params = LightingBakeParams {
        probe_spacing: 10.0,
        ..Default::default()
    };
    let bake = |scene: &mut Scene| {
        bake_lighting(
            scene,
            Some(scene_path),
            None,
            params,
            &mut |_| {},
            &mut BakeLog::off(),
        )
        .expect("rebake never errors with a scene path")
    };
    let first = bake(&mut scene);
    let second = bake(&mut scene);
    assert!(first.auto_placed_light && first.auto_placed_reflection);
    assert_eq!(first, second, "a re-placement places the same");
    if !first.ran_light_bake {
        return;
    }
    assert_manual_kept(&scene, &manual);
}

fn with_manual_probes() -> (Scene, rusty_scene::ReflectionProbe) {
    let mut scene = room_scene();
    scene.lighting_settings.reflections.resolution = 16;
    let light = scene.probes.add_probe(Vec3::new(1.0, 4.0, 1.0));
    scene.probes.set_manual(light, true);
    scene.probes.add_probe(Vec3::new(-9.0, 4.0, -9.0));
    let set = &mut scene.reflection_probes;
    let mine = set.add(Vec3::new(5.0, 4.6, 5.0), Vec3::splat(3.0));
    set.edit(mine, |p| (p.manual, p.intensity) = (true, 2.5));
    let manual = set.probes[mine].clone();
    (scene, manual)
}

fn assert_manual_kept(scene: &Scene, manual: &rusty_scene::ReflectionProbe) {
    assert_eq!(scene.probes.probes()[0].position, Vec3::new(1.0, 4.0, 1.0));
    assert!(scene.probes.probes()[0].manual);
    assert_eq!(scene.probes.probes().iter().filter(|p| p.manual).count(), 1);
    let old = Vec3::new(-9.0, 4.0, -9.0);
    assert!(
        !scene.probes.probes().iter().any(|p| p.position == old),
        "the old placed probe is replaced"
    );
    let kept = &scene.reflection_probes.probes[0];
    assert_eq!(
        (
            kept.position,
            kept.box_min,
            kept.box_max,
            kept.intensity,
            kept.manual
        ),
        (
            manual.position,
            manual.box_min,
            manual.box_max,
            manual.intensity,
            true
        ),
    );
    for p in &scene.reflection_probes.probes[1..] {
        assert!(!p.manual);
        assert!(
            !manual.contains(p.position),
            "the manual probe's room is its own"
        );
    }
}

#[test]
fn empty_scene_places_nothing_and_succeeds() {
    let mut scene = Scene::new();
    let report = bake_lighting(
        &mut scene,
        None,
        None,
        LightingBakeParams::default(),
        &mut |_| {},
        &mut BakeLog::off(),
    )
    .expect("no-op bake");
    assert_eq!(report.light_probes, 0);
    assert_eq!(report.reflection_probes, 0);
    assert!(!report.auto_placed_light);
    assert!(!report.auto_placed_reflection);
}
