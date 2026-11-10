//! The reflection probes' report: which probe each object picks (two probes, an
//! object in each box and one between), and their overlaps. The picks need no
//! GPU; the stepped bake writing both probe reports does, and skips without one.

use glam::Vec3;

use super::{assert_common, scene_path};
use crate::dev::bake::report::{read, reflections, Kind, Stages};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::Scene;

/// Two 4 m rooms side by side, touching at `x = 0` and overlapping by 1 m, with a
/// box in each and one outside both.
fn rooms() -> Scene {
    let mut scene = Scene::new();
    for (x, name) in [(-2.5, "West"), (2.5, "East"), (40.0, "Outside")] {
        let id = create_entity(&mut scene, name, Some(Primitive::Box));
        scene.world.transform_mut(id).expect("new").position = Vec3::new(x, 1.0, 0.0);
    }
    let set = &mut scene.reflection_probes;
    let west = set.add(Vec3::new(-2.5, 1.0, 0.0), Vec3::splat(2.0));
    let east = set.add(Vec3::new(2.5, 1.0, 0.0), Vec3::splat(2.0));
    set.set_box(west, Vec3::new(-5.0, 0.0, -2.0), Vec3::new(0.5, 3.0, 2.0));
    set.set_box(east, Vec3::new(-0.5, 0.0, -2.0), Vec3::new(5.0, 3.0, 2.0));
    scene
}

#[test]
fn each_object_reports_the_probe_it_picks() {
    let scene = rooms();
    let picks = reflections::picks(&scene, &scene.reflection_probes);
    let probe = |name: &str| {
        let pick = picks.as_array().unwrap().iter().find(|p| p["name"] == name);
        pick.unwrap_or_else(|| panic!("{name} picked nothing: {picks}"))["probe"].clone()
    };
    assert_eq!(probe("West"), 0);
    assert_eq!(probe("East"), 1);
    assert!(probe("Outside").is_null(), "outside every box: the sky");
}

#[test]
fn the_report_lists_each_probe_with_its_overlaps() {
    let scene = rooms();
    let dir = std::path::PathBuf::from(scene_path("overlaps")).with_extension("reflections");
    let report = reflections::report(&scene, &Stages::start(), &[], &dir).to_json();
    assert_common(&report, Kind::Reflections);
    assert_eq!(report["count"], 2);
    assert_eq!(report["probes"][0]["overlaps"], serde_json::json!([1]));
    assert_eq!(report["probes"][1]["overlaps"], serde_json::json!([0]));
    assert!(report["probes"][0]["seconds"].is_null(), "not timed");
    assert_eq!(report["picks"].as_array().unwrap().len(), 3);
}

#[test]
fn gpu_a_probe_bake_writes_both_reports() {
    use crate::dev::bake::lighting::{run_to_end, start_lighting, LightingBakeParams};
    let path = scene_path("probe_bake");
    let mut scene = rooms();
    scene.lighting_settings.reflections.resolution = 16;
    for i in 0..2 {
        scene.reflection_probes.edit(i, |p| p.manual = true);
    }
    scene.probes.add_probe(Vec3::new(0.0, 1.0, 0.0));
    scene.probes.set_manual(0, true);
    let params = LightingBakeParams::default();
    let mut job = start_lighting(&scene, Some(&path), None, params).unwrap();
    run_to_end(
        &mut job,
        &mut scene,
        &mut |_| {},
        &mut crate::dev::bake::progress::BakeLog::off(),
    )
    .unwrap();
    job.commit(&mut scene).unwrap();
    if !job.report().ran_reflection_bake {
        eprintln!("no GPU adapter: skipping");
        return;
    }
    let reflections = read(&path, Kind::Reflections).expect("the reflections report");
    assert_common(&reflections, Kind::Reflections);
    let probe = &reflections["probes"][0];
    assert!(
        probe["bytes"].as_u64() > Some(0) && probe["seconds"].as_f64().is_some(),
        "{probe}"
    );
    let probes = read(&path, Kind::Probes).expect("the light probes' report");
    assert_common(&probes, Kind::Probes);
    assert_eq!(
        (probes["count"].as_u64(), probes["manual"].as_u64()),
        (Some(1), Some(1))
    );
    assert!(
        !probes["bounces"].as_array().unwrap().is_empty(),
        "{probes}"
    );
}
