//! Tests for the one-button lighting bake orchestration (#246). Sibling of
//! `lighting.rs`; shares its 300-line source cap.
//!
//! These cover the PLACEMENT + ORCHESTRATION contract, which runs adapter-free: the
//! GPU bakes skip gracefully when no adapter is present (`ran_*` = false), but the
//! auto-placement that precedes them still populates both probe sets. The deep GPU
//! bake behaviour is owned by `render::probe_bake_tests` / `render::reflection_bake_tests`.

use glam::Vec3;

use super::{bake_lighting, LightingBakeParams};
use crate::components::{ColliderComponent, ColliderShape};
use crate::dev::bake::progress::BakeLog;
use crate::scene::Scene;

/// A static room box spanning ±10 in XZ, 6 tall, with a box collider so the placement
/// has static bounds to work from.
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
            ..crate::scene::authoring::default_collider()
        }),
    );
    scene
}

/// On a static scene with no probes, the bake auto-places BOTH sets. The GPU bakes
/// skip without an adapter, but placement has already populated the world — the
/// "place well + bake right now" acceptance, minus the adapter-bound bake itself.
#[test]
fn gpu_auto_places_both_sets_on_empty_scene() {
    let dir = crate::test_temp::dir().join("rusty_lighting_bake_test");
    let _ = std::fs::create_dir_all(&dir);
    let scene_path = dir.join("room.scene");
    let scene_path = scene_path.to_str().unwrap();

    let mut scene = room_scene();
    // A coarse grid and one reflection region: the claim is that both sets get
    // placed, which a few probes prove as well as the default grid (#985).
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
    assert_eq!(scene.probes.probes.len(), report.light_probes);
    assert_eq!(scene.reflection_probes.len(), report.reflection_probes);
}

/// Manual probes survive a re-placement unchanged (#1737): every bake replaces
/// only the placed probes, keeps the manual ones with their position, box and
/// settings, and leaves the room a manual reflection probe covers to it.
#[test]
fn gpu_manual_probes_survive_a_replacement() {
    let dir = crate::test_temp::dir().join("rusty_lighting_bake_test");
    let _ = std::fs::create_dir_all(&dir);
    let scene_path = dir.join("manual.scene");
    let scene_path = scene_path.to_str().unwrap();

    let mut scene = room_scene();
    scene.lighting_settings.reflections.resolution = 16;
    let light = scene.probes.add_probe(Vec3::new(1.0, 4.0, 1.0));
    scene.probes.set_manual(light, true);
    // A probe the placement made in an earlier bake: it is replaced.
    scene.probes.add_probe(Vec3::new(-9.0, 4.0, -9.0));
    let set = &mut scene.reflection_probes;
    let mine = set.add(Vec3::new(5.0, 4.6, 5.0), Vec3::splat(3.0));
    set.edit(mine, |p| (p.manual, p.intensity) = (true, 2.5));
    let manual = set.probes[mine].clone();

    let params = LightingBakeParams {
        probe_spacing: 10.0,
        ..Default::default()
    };
    let mut bake = |scene: &mut Scene| {
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
    assert_eq!(scene.probes.probes[0].position, Vec3::new(1.0, 4.0, 1.0));
    assert!(scene.probes.probes[0].manual);
    assert_eq!(scene.probes.probes.iter().filter(|p| p.manual).count(), 1);
    assert!(
        !(scene.probes.probes.iter()).any(|p| p.position == Vec3::new(-9.0, 4.0, -9.0)),
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

/// An empty scene (no static geometry) places nothing and still succeeds — both bakes
/// are no-ops over empty sets.
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
