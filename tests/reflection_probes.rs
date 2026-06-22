//! Reflection probes (#244): data model + parallax-corrected runtime sampling.
//!
//! Headless + deterministic, no GPU. Asserts (1) the reflection-probe set — positions,
//! parallax boxes, and cubemap PATHS — round-trips through the scene document, and
//! (2) the nearest-probe selection + box-projected parallax math behaves: a surface
//! inside a probe's box reflects that probe (tracking the room as the camera moves),
//! and points outside every box fall back (no probe → skybox).

use glam::Vec3;
use rusty::scene::Scene;

fn tmp(name: &str) -> String {
    std::env::temp_dir()
        .join(name)
        .to_string_lossy()
        .into_owned()
}

/// Positions, parallax boxes, and cubemap paths all live in the scene document and
/// survive a save → load round-trip. No sidecar: the cubemap is a referenced file.
#[test]
fn reflection_probe_set_round_trips() {
    let mut scene = Scene::new();
    let i = scene
        .reflection_probes
        .add(Vec3::new(1.0, 2.0, 3.0), Vec3::splat(5.0));
    scene
        .reflection_probes
        .set_box(i, Vec3::new(-4.0, -4.0, -4.0), Vec3::new(6.0, 6.0, 6.0));
    scene
        .reflection_probes
        .set_cubemap(i, "reflections/room.ktx2".to_string());

    let path = tmp("rusty_reflection_probes.scene");
    scene.save_to_file(&path).unwrap();

    // Positions + box corners + the cubemap PATH are in the human-diffable scene doc.
    let scene_json = std::fs::read_to_string(&path).unwrap();
    assert!(scene_json.contains("\"reflection_probes\""));
    assert!(scene_json.contains("reflections/room.ktx2"));

    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    assert_eq!(
        loaded.reflection_probes.probes,
        scene.reflection_probes.probes
    );
    let p = &loaded.reflection_probes.probes[0];
    assert_eq!(p.position, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(p.box_min, Vec3::new(-4.0, -4.0, -4.0));
    assert_eq!(p.box_max, Vec3::new(6.0, 6.0, 6.0));
    assert_eq!(p.cubemap_path, "reflections/room.ktx2");
}

/// The acceptance selection logic: of two boxes covering a point, the nearer probe
/// wins; a point outside every box selects nothing (caller falls back to the skybox).
#[test]
fn nearest_containing_probe_is_selected() {
    let mut scene = Scene::new();
    scene
        .reflection_probes
        .add(Vec3::new(8.0, 0.0, 0.0), Vec3::splat(10.0)); // covers origin, far
    scene
        .reflection_probes
        .add(Vec3::new(2.0, 0.0, 0.0), Vec3::splat(10.0)); // covers origin, near

    assert_eq!(scene.reflection_probes.select_index(Vec3::ZERO), Some(1));
    assert!(scene
        .reflection_probes
        .select(Vec3::new(100.0, 0.0, 0.0))
        .is_none());
}

/// Box-projected parallax: as the SAME surface point is reflected from a probe, the
/// corrected direction points at the box wall the ray actually hits — so moving the
/// fragment within the room changes what is reflected, unlike an infinite skybox.
#[test]
fn parallax_tracks_the_room() {
    let mut scene = Scene::new();
    let i = scene.reflection_probes.add(Vec3::ZERO, Vec3::splat(4.0));
    let probe = &scene.reflection_probes.probes[i];

    // A fragment at the box centre reflecting straight +X hits the +X wall: corrected
    // direction is +X.
    let centre = probe.parallax_correct(Vec3::ZERO, Vec3::X);
    assert!((centre - Vec3::X).length() < 1e-4, "centre: {centre:?}");

    // The same +X reflection from an OFF-centre fragment bends toward the wall point —
    // the room-tracking behaviour. The hit is (4, y, 0); the corrected dir differs.
    let offset = probe.parallax_correct(Vec3::new(0.0, 2.0, 0.0), Vec3::X);
    assert!(
        (offset - Vec3::X).length() > 1e-3,
        "off-centre reflection should bend: {offset:?}"
    );
    assert!(offset.is_finite());
}
