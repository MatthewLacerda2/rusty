//! Tests for the one-button lighting bake orchestration (#246). Sibling of
//! `lighting_bake.rs`; shares its 300-line source cap.
//!
//! These cover the PLACEMENT + ORCHESTRATION contract, which runs adapter-free: the
//! GPU bakes skip gracefully when no adapter is present (`ran_*` = false), but the
//! auto-placement that precedes them still populates both probe sets. The deep GPU
//! bake behaviour is owned by `render::probe_bake_tests` / `render::reflection_bake_tests`.

use glam::Vec3;

use super::{bake_lighting, LightingBakeParams};
use crate::components::{ColliderComponent, ColliderShape};
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
            active: true,
            shape: ColliderShape::Box {
                size: Vec3::new(20.0, 6.0, 20.0),
            },
            is_trigger: false,
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        }),
    );
    scene.update_entity_collider(id);
    scene
}

/// On a static scene with no probes, the bake auto-places BOTH sets. The GPU bakes
/// skip without an adapter, but placement has already populated the world — the
/// "place well + bake right now" acceptance, minus the adapter-bound bake itself.
#[test]
fn auto_places_both_sets_on_empty_scene() {
    let dir = std::env::temp_dir().join("rusty_lighting_bake_test");
    let _ = std::fs::create_dir_all(&dir);
    let scene_path = dir.join("room.scene");
    let scene_path = scene_path.to_str().unwrap();

    let mut scene = room_scene();
    let report = bake_lighting(
        &mut scene,
        Some(scene_path),
        None,
        LightingBakeParams::default(),
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

/// Manually-authored probes are NOT clobbered: a scene that already carries a probe in
/// each set is rebaked in place, with no auto-placement (the documented per-set rule).
#[test]
fn manual_probes_are_kept_not_replaced() {
    let dir = std::env::temp_dir().join("rusty_lighting_bake_test");
    let _ = std::fs::create_dir_all(&dir);
    let scene_path = dir.join("manual.scene");
    let scene_path = scene_path.to_str().unwrap();

    let mut scene = room_scene();
    scene.probes.add_probe(Vec3::new(1.0, 1.0, 1.0));
    scene
        .reflection_probes
        .add(Vec3::new(2.0, 2.0, 2.0), Vec3::splat(3.0));

    let report = bake_lighting(
        &mut scene,
        Some(scene_path),
        None,
        LightingBakeParams::default(),
    )
    .expect("rebake never errors with a scene path");

    assert!(!report.auto_placed_light, "manual light set kept");
    assert!(!report.auto_placed_reflection, "manual reflection set kept");
    assert_eq!(report.light_probes, 1);
    assert_eq!(report.reflection_probes, 1);
}

/// An empty scene (no static geometry) places nothing and still succeeds — both bakes
/// are no-ops over empty sets.
#[test]
fn empty_scene_places_nothing_and_succeeds() {
    let mut scene = Scene::new();
    let report =
        bake_lighting(&mut scene, None, None, LightingBakeParams::default()).expect("no-op bake");
    assert_eq!(report.light_probes, 0);
    assert_eq!(report.reflection_probes, 0);
    assert!(!report.auto_placed_light);
    assert!(!report.auto_placed_reflection);
}
