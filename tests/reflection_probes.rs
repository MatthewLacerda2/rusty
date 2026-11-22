use crate::temp::path as tmp;
use glam::Vec3;
use rusty::scene::Scene;

#[test]
fn reflection_probe_set_round_trips() {
    let mut scene = Scene::new();
    let i = scene
        .reflection_probes
        .add(Vec3::new(1.0, 2.0, 3.0), Vec3::splat(5.0));
    scene
        .reflection_probes
        .set_box(i, Vec3::new(-4.0, -4.0, -4.0), Vec3::new(6.0, 6.0, 6.0));
    scene.reflection_probes.edit(i, |p| {
        (p.importance, p.resolution, p.shadows) = (4, Some(128), false);
    });
    scene
        .reflection_probes
        .set_cubemap(i, "reflections/room.ktx2".to_string());

    let path = tmp("rusty_reflection_probes.scene");
    rusty::edit::save_to_file(&scene, &path).unwrap();

    let scene_json = std::fs::read_to_string(&path).unwrap();
    assert!(scene_json.contains("\"reflection_probes\""));
    assert!(scene_json.contains("reflections/room.ktx2"));

    let mut loaded = Scene::new();
    rusty::edit::load_from_file(&mut loaded, &path).unwrap();
    assert_eq!(
        loaded.reflection_probes.probes,
        scene.reflection_probes.probes
    );
    let p = &loaded.reflection_probes.probes[0];
    assert_eq!(p.position, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(p.box_min, Vec3::new(-4.0, -4.0, -4.0));
    assert_eq!(p.box_max, Vec3::new(6.0, 6.0, 6.0));
    assert_eq!(p.cubemap_path, "reflections/room.ktx2");
    assert_eq!(
        (p.importance, p.resolution, p.shadows),
        (4, Some(128), false)
    );
}

#[test]
fn covering_probes_blend_nearest_first() {
    let mut scene = Scene::new();
    scene
        .reflection_probes
        .add(Vec3::new(8.0, 0.0, 0.0), Vec3::splat(10.0)); // covers origin, far
    scene
        .reflection_probes
        .add(Vec3::new(2.0, 0.0, 0.0), Vec3::splat(10.0)); // covers origin, near

    let blend = scene.reflection_probes.blend(Vec3::ZERO).unwrap();
    assert_eq!((blend.first, blend.second), (1, Some(0)));
    assert_eq!(blend.first_weight + blend.second_weight, 1.0);
    assert!(scene
        .reflection_probes
        .blend(Vec3::new(100.0, 0.0, 0.0))
        .is_none());
}

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
