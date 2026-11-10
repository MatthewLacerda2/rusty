//! The light probes' report: the dynamic objects sampling far from any probe, and
//! the timeline that splits a stepped bake into bounces and reflection probes.

use glam::Vec3;

use super::assert_common;
use crate::dev::bake::report::probes::{report, Timeline, FAR};
use crate::dev::bake::report::Kind;
use crate::dev::bake::steps::Stage;
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::Scene;

#[test]
fn a_dynamic_object_far_from_every_probe_is_listed() {
    let mut scene = Scene::new();
    scene.probes.add_probe(Vec3::ZERO);
    scene.probes.add_probe(Vec3::new(2.0, 0.0, 0.0));
    for (x, name, is_static) in [
        (1.0, "Near", false),
        (30.0, "Far", false),
        (60.0, "Wall", true),
    ] {
        let id = create_entity(&mut scene, name, Some(Primitive::Box));
        scene.world.transform_mut(id).expect("new").position = Vec3::new(x, 0.0, 0.0);
        scene.world.set_static(id, is_static);
    }
    let report = report(&scene, &crate::dev::bake::report::Stages::start()).to_json();
    assert_common(&report, Kind::Probes);
    assert_eq!(report["count"], 2);
    assert_eq!(report["bounds"]["max"], serde_json::json!([2.0, 0.0, 0.0]));
    let far = &report["farObjects"];
    assert_eq!(
        far["count"], 1,
        "the static wall is lightmapped, not probe-lit: {far}"
    );
    assert_eq!(far["list"][0]["name"], "Far");
    let nearest = far["list"][0]["nearest"].as_f64().unwrap();
    assert!(
        nearest > FAR as f64 && (nearest - 28.0).abs() < 0.01,
        "{nearest}"
    );
}

#[test]
fn a_probe_set_with_no_probes_reports_nothing_near() {
    let mut scene = Scene::new();
    create_entity(&mut scene, "Crate", Some(Primitive::Box));
    let report = report(&scene, &crate::dev::bake::report::Stages::start()).to_json();
    assert!(report["bounds"]["min"].is_null());
    assert!(
        report["farObjects"]["list"][0]["nearest"].is_null(),
        "no probe at all"
    );
}

#[test]
fn the_timeline_gives_each_stage_its_own_time() {
    let mut t = Timeline::default();
    let light = |bounce| Stage::Light {
        bounce,
        done: 0,
        total: 4,
    };
    let reflection = |done| Stage::Reflection { done, total: 2 };
    for stage in [
        light(1),
        light(1),
        light(2),
        reflection(0),
        reflection(1),
        Stage::Done,
    ] {
        t.note(stage);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!((t.bounces.len(), t.reflections.len()), (2, 2));
    assert!(t.bounces[0] > t.bounces[1], "bounce 1 had two steps");
    let (light, reflection) = t.stages();
    let names: Vec<_> = light
        .to_json()
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].clone())
        .collect();
    assert_eq!(names, ["bounce 1", "bounce 2"]);
    assert_eq!(reflection.total(), t.reflections.iter().sum());
}
