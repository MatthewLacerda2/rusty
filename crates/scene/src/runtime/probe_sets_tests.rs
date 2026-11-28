use glam::Vec3;

use super::SceneProbes;
use crate::Scene;

/// A World of `base.scene` (a light probe at the origin) and `b.scene` joined beside it (one at
/// x = 10 and a reflection probe there).
fn two_sets() -> (Scene, u32) {
    let mut scene = Scene::new();
    scene.probes.add_probe(Vec3::ZERO);
    scene.reflection_probes.add(Vec3::ZERO, Vec3::ONE);
    let settings = scene.settings();
    let base = Some("base.scene".to_string());
    let b = scene.join_loaded(base, "b.scene".into(), settings, &[]);
    let mut probes = SceneProbes::default();
    probes.light.add_probe(Vec3::X * 10.0);
    probes.reflection.add(Vec3::X * 10.0, Vec3::ONE);
    scene.park_probes(b, probes);
    (scene, b)
}

fn xs(scene: &Scene) -> Vec<(u32, Vec<f32>)> {
    let sets = scene.probe_sets().into_iter();
    let x =
        |v: &crate::lighting::probe::ProbeVolume| v.probes().iter().map(|p| p.position.x).collect();
    sets.map(|(h, v)| (h, x(v))).collect()
}

#[test]
fn each_loaded_scene_keeps_its_own_set_in_load_order() {
    let (scene, b) = two_sets();
    assert_eq!(xs(&scene), [(0, vec![0.0]), (b, vec![10.0])]);
    assert_eq!(
        scene.probes.probes().len(),
        1,
        "the base's set is not renumbered"
    );
    let reflections = scene.all_reflection_probes();
    assert_eq!(reflections.len(), 2, "both scenes' cubes are candidates");
}

#[test]
fn activation_moves_the_sets_and_keeps_their_revisions() {
    let (mut scene, b) = two_sets();
    let before = scene.with_probe_field(|f| f.revisions());
    scene.set_active_scene(b);
    assert_eq!(
        scene.probes.probes()[0].position.x,
        10.0,
        "b's set is the editable one"
    );
    assert_eq!(
        xs(&scene),
        [(0, vec![0.0]), (b, vec![10.0])],
        "the field is unchanged"
    );
    assert_eq!(
        scene.with_probe_field(|f| f.revisions()),
        before,
        "nothing re-samples"
    );
    scene.set_active_scene(0);
    assert_eq!(scene.probes.probes()[0].position.x, 0.0);
}

#[test]
fn an_unload_takes_its_sets_with_it() {
    let (mut scene, b) = two_sets();
    scene.unload_scene(b).unwrap();
    assert_eq!(xs(&scene), [(0, vec![0.0])]);
    assert_eq!(scene.all_reflection_probes().len(), 1);
    let (mut scene, b) = two_sets();
    scene.set_active_scene(b);
    scene.unload_scene(0).unwrap();
    assert_eq!(
        xs(&scene),
        [(b, vec![10.0])],
        "the active one leaving hands over"
    );
}

#[test]
fn a_single_load_drops_every_parked_set() {
    let (mut scene, _) = two_sets();
    scene.reset_loaded();
    assert_eq!(xs(&scene), [(0, vec![0.0])]);
}
