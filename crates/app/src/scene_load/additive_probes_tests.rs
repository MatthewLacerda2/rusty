use glam::Vec3;
use rusty_core::paths;
use rusty_scene::lighting::io::{read_lighting_sidecar, ProbeKey};
use rusty_scene::Scene;

use super::fixture::world_at;
use super::GameWorld;

const DT: f32 = 1.0 / 60.0;

/// `base.scene`: eight baked probes along x from 0, DC 1. `b.scene`: four from x = 20, DC 100.
/// Each has a reflection probe of its own cube.
fn scenes(tag: &str) -> [String; 2] {
    let dir = rusty_test_support::temp::dir().join(format!("rusty_2504_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let write = |name: &str, from: f32, n: usize, dc: f32| {
        let mut scene = Scene::new();
        scene.add_entity(name.to_string());
        for k in 0..n {
            let i = scene.probes.add_probe(Vec3::new(from + k as f32, 1.0, 0.0));
            scene.probes.probes_mut()[i].sh.coeffs[0] = [dc, dc, dc];
        }
        let r = scene.reflection_probes.add(Vec3::X * from, Vec3::ONE);
        scene.reflection_probes.probes[r].cubemap_path = format!("{name}.ktx2");
        let path = paths::to_slash(&dir.join(format!("{name}.scene")));
        rusty_edit::save_to_file(&scene, &path).unwrap();
        path
    };
    [write("base", 0.0, 8, 1.0), write("b", 20.0, 4, 100.0)]
}

fn with_b(tag: &str) -> (GameWorld, [String; 2]) {
    let paths = scenes(tag);
    let (mut gw, _) = world_at(&paths[0]);
    gw.set_playing(true);
    gw.tick(DT);
    let load = format!("Scene.Load('{}', 'Additive')", paths[1]);
    gw.script_manager().eval(&load).unwrap();
    gw.tick(DT);
    (gw, paths)
}

fn dc(gw: &GameWorld, x: f32) -> f32 {
    let scene = gw.scene().borrow();
    scene.sample_probes(Vec3::new(x, 1.0, 0.0)).unwrap().coeffs[0][0]
}

#[test]
fn each_scene_brings_its_baked_probes_keyed_by_its_own_set() {
    let (gw, paths) = with_b("keys");
    let scene = gw.scene().borrow();
    let sets = scene.probe_sets();
    assert_eq!(sets.len(), 2, "a set per loaded scene");
    for ((_, set), path) in sets.iter().zip(&paths) {
        let file = read_lighting_sidecar(path);
        assert!(file.has_probes());
        let mut alone = Scene::new();
        alone.probes = (*set).clone();
        rusty_scene::lighting::io::apply_lighting_sidecar(&mut alone, file).unwrap();
        assert_eq!(ProbeKey::of(&alone), ProbeKey::of_set(set), "{path}");
    }
    assert_eq!(
        scene.probes.probes().len(),
        8,
        "the World's set is not renumbered"
    );
    assert_eq!(dc(&gw, 21.0), 100.0, "b's SH is applied");
    let cubes: Vec<&str> = scene
        .all_reflection_probes()
        .iter()
        .map(|p| p.cubemap_path.as_str())
        .collect();
    assert_eq!(
        cubes,
        ["base.ktx2", "b.ktx2"],
        "both scenes' cubes are candidates"
    );
}

#[test]
fn a_point_between_is_lit_by_both_and_an_unload_moves_only_what_stood_near() {
    let (mut gw, paths) = with_b("unload");
    let between = dc(&gw, 13.5);
    assert!(between > 1.0 && between < 100.0, "{between}");
    let far = gw.scene().borrow().sample_probes(Vec3::new(2.0, 1.0, 0.0));
    let near = dc(&gw, 18.0);
    gw.script_manager()
        .eval(&format!("Scene.Unload('{}')", paths[1]))
        .unwrap();
    gw.tick(DT);
    assert_eq!(
        gw.scene().borrow().probe_sets().len(),
        1,
        "b's set left with it"
    );
    let after = gw.scene().borrow().sample_probes(Vec3::new(2.0, 1.0, 0.0));
    assert_eq!(after, far, "a far point's SH is bit-identical");
    assert_ne!(dc(&gw, 18.0), near, "a point near b's probes changed");
    assert_eq!(gw.scene().borrow().all_reflection_probes().len(), 1);
}
