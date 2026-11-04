//! A stepped bake applies its planned sets only at its commit (#1737): dropped
//! midway or failing to write a cube, the scene keeps its old lighting, and while it
//! runs dynamic objects keep sampling the old field. Skips without an adapter.

use glam::Vec3;

use super::super::lighting::start_lighting;
use super::staging_tests::{assert_old_lighting, lit_room, old, params, saved_path};
use super::{LightingBake, PROBES_CHANGED};
use crate::scene::Scene;

/// Midway through a stepped bake the scene still samples its old field; an edit
/// then drops the bake with the old lighting kept.
#[test]
fn gpu_a_bake_dropped_midway_keeps_the_old_lighting() {
    let mut scene = lit_room();
    let was = old(&scene);
    let Some(mut job) = started(&scene, "dropped") else {
        return;
    };
    job.step(&mut scene, &mut |_| {}).unwrap();
    assert_old_lighting(&scene, &was);
    scene.probes.add_probe(Vec3::ONE);
    let edited = old(&scene);
    assert_eq!(
        job.step(&mut scene, &mut |_| {}),
        Err(PROBES_CHANGED.into())
    );
    assert_eq!(job.commit(&mut scene), Err(PROBES_CHANGED.into()));
    assert_old_lighting(&scene, &edited);
}

/// A cube that cannot be written fails the bake with the old lighting kept.
#[test]
fn gpu_a_cube_write_failure_keeps_the_old_lighting() {
    let mut scene = lit_room();
    let was = old(&scene);
    let gone = crate::test_temp::dir().join("rusty_1737_missing");
    let gone = gone.join("nowhere").join("level.scene");
    let gone = gone.to_string_lossy().into_owned();
    let job = start_lighting(&scene, Some(&gone), None, params()).unwrap();
    if !job.report().ran_reflection_bake {
        return; // no adapter
    }
    let mut job = job;
    let failed = loop {
        match job.step(&mut scene, &mut |_| {}) {
            Ok(true) => panic!("baked into a directory that does not exist"),
            Ok(false) => continue,
            Err(e) => break e,
        }
    };
    assert!(failed.contains("reflection cubemap"), "{failed}");
    assert_old_lighting(&scene, &was);
}

/// A finished bake swaps the planned sets in, lit.
#[test]
fn gpu_the_commit_applies_the_planned_sets() {
    let mut scene = lit_room();
    let Some(mut job) = started(&scene, "commit") else {
        return;
    };
    while !job.step(&mut scene, &mut |_| {}).unwrap() {}
    job.commit(&mut scene).unwrap();
    let report = job.report();
    assert_eq!(scene.probes.probes.len(), report.light_probes);
    assert_eq!(scene.reflection_probes.len(), report.reflection_probes);
    assert!(scene
        .reflection_probes
        .probes
        .iter()
        .all(|p| p.cubemap_path != "old.ktx2"));
    assert!(scene
        .probes
        .probes
        .iter()
        .any(|p| p.sh != crate::scene::lighting::sh::Sh9::zero()));
}

/// The bake of `lit_room` saved under `name`; `None` without an adapter.
fn started(scene: &Scene, name: &str) -> Option<LightingBake> {
    let job = start_lighting(scene, Some(&saved_path(name)), None, params()).unwrap();
    job.report().ran_reflection_bake.then_some(job)
}
