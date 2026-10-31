//! A stepped bake works on a snapshot and applies only when it completes (#1602):
//! nothing reaches the scene before the commit, and a probe set changed midway drops
//! the bake with nothing applied and no staged file left behind.

use glam::Vec3;

use super::super::lighting::{start_lighting, LightingBakeParams};
use super::tests::probed;
use super::{LightingBake, Stage, PROBES_CHANGED};
use crate::scene::lighting::sh::Sh9;
use crate::scene::Scene;

fn start(scene: &mut Scene, path: &str) -> Option<LightingBake> {
    let job = start_lighting(scene, Some(path), None, LightingBakeParams::default()).unwrap();
    job.report().ran_reflection_bake.then_some(job) // None: no adapter
}

/// Step until `until` holds or the bake is done.
fn step_until(job: &mut LightingBake, scene: &mut Scene, until: impl Fn(Stage) -> bool) {
    while !until(job.stage()) && !job.step(scene, &mut |_| {}).unwrap() {}
}

fn staged_files(path: &str) -> usize {
    let dir = std::path::Path::new(path).parent().unwrap();
    let names = std::fs::read_dir(dir).unwrap().flatten();
    names
        .filter(|e| e.file_name().to_string_lossy().ends_with(".baking"))
        .count()
}

#[test]
fn gpu_nothing_reaches_the_scene_until_the_commit() {
    let (mut scene, path) = probed("snapshot_commit", 2, 2);
    let Some(mut job) = start(&mut scene, &path) else {
        return;
    };
    step_until(&mut job, &mut scene, |s| s == Stage::Done);
    assert!(scene.probes.probes.iter().all(|p| p.sh == Sh9::zero()));
    assert!(scene
        .reflection_probes
        .probes
        .iter()
        .all(|p| p.cubemap_path.is_empty()));
    assert_eq!(staged_files(&path), 2, "both cubes staged");
    job.commit(&mut scene).unwrap();
    assert!(scene.probes.probes.iter().all(|p| p.sh != Sh9::zero()));
    let baked = &scene.reflection_probes.probes;
    assert!(baked
        .iter()
        .all(|p| std::path::Path::new(&p.cubemap_path).exists()));
    assert_eq!(staged_files(&path), 0, "moved into place");
}

/// Removing a reflection probe midway used to skip one and report success, or bake
/// probe i over another probe's file.
#[test]
fn gpu_a_reflection_probe_removed_midway_drops_the_bake() {
    let (mut scene, path) = probed("snapshot_reflection", 1, 3);
    let Some(mut job) = start(&mut scene, &path) else {
        return;
    };
    step_until(&mut job, &mut scene, |s| {
        matches!(s, Stage::Reflection { done: 1, .. })
    });
    scene.reflection_probes.remove(0);
    assert_eq!(
        job.step(&mut scene, &mut |_| {}),
        Err(PROBES_CHANGED.into())
    );
    assert_eq!(job.commit(&mut scene), Err(PROBES_CHANGED.into()));
    drop(job);
    assert_eq!(staged_files(&path), 0, "the staged cube went with the job");
    assert!(scene
        .reflection_probes
        .probes
        .iter()
        .all(|p| p.cubemap_path.is_empty()));
    assert!(
        scene.probes.probes.iter().all(|p| p.sh == Sh9::zero()),
        "nothing applied"
    );
}

/// `Probe.Remove(0)` or a `FillGrid` midway used to shift every probe's SH by one.
#[test]
fn gpu_a_light_probe_removed_or_regridded_midway_drops_the_bake() {
    for edit in [
        (|s: &mut Scene| s.probes.remove_probe(0)) as fn(&mut Scene),
        |s| s.probes.fill_grid(Vec3::ZERO, Vec3::splat(4.0), 2.0),
    ] {
        let (mut scene, path) = probed("snapshot_light", 6, 0);
        let Some(mut job) = start(&mut scene, &path) else {
            return;
        };
        job.step(&mut scene, &mut |_| {}).unwrap();
        edit(&mut scene);
        assert_eq!(
            job.step(&mut scene, &mut |_| {}),
            Err(PROBES_CHANGED.into())
        );
        assert!(scene.probes.probes.iter().all(|p| p.sh == Sh9::zero()));
    }
}

/// An edit the cubes are not baked from (a box, an intensity) does not drop the bake,
/// and survives the commit.
#[test]
fn gpu_an_edit_the_bake_does_not_read_survives_the_commit() {
    let (mut scene, path) = probed("snapshot_box", 0, 1);
    let Some(mut job) = start(&mut scene, &path) else {
        return;
    };
    scene.reflection_probes.probes[0].intensity = 2.0;
    step_until(&mut job, &mut scene, |s| s == Stage::Done);
    job.commit(&mut scene).unwrap();
    let probe = &scene.reflection_probes.probes[0];
    assert_eq!(probe.intensity, 2.0);
    assert!(!probe.cubemap_path.is_empty());
}
