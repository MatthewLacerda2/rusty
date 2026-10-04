//! The editor's background bake (#808): it applies what the synchronous bake would,
//! reports progress, and a cancelled bake leaves the previous lightmaps alone.

use std::time::Duration;

use super::tests::{quick, scene, scene_path};
use super::*;

/// Poll `job` until its worker has finished (a debug-build bake of the test scene
/// takes well under the bound).
fn finish(job: &mut LightmapBakeJob, scene: &mut Scene) -> BakeOutcome {
    for _ in 0..12_000 {
        if let Some(outcome) = job.poll(scene) {
            return outcome;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the bake never finished");
}

#[test]
fn a_background_bake_applies_what_the_synchronous_one_does() {
    let (mut sync, floor, _) = scene();
    let path = scene_path("job_sync");
    bake_scene_lightmaps(&mut sync, Some(&path), &quick()).unwrap();
    let synchronous = std::fs::read(&sync.lightmaps.pages[0]).unwrap();

    let (mut scene, _, _) = scene();
    let path = scene_path("job_async");
    let mut job = LightmapBakeJob::start(&mut scene, Some(&path), quick()).unwrap();
    assert_eq!(job.scene_path(), path);
    assert_eq!(finish(&mut job, &mut scene), BakeOutcome::Baked(1));
    assert!(scene.lightmaps.get(floor).is_some());
    let background = std::fs::read(&scene.lightmaps.pages[0]).unwrap();
    assert_eq!(background, synchronous, "byte for byte");

    let (done, total) = job.progress();
    assert!(total > 0 && done == total);
    assert_eq!(job.fraction(), 1.0);
    assert_eq!(job.poll(&mut scene), None, "a spent job reports once");
}

#[test]
fn a_cancelled_bake_keeps_the_previous_lightmaps() {
    let (mut scene, _, _) = scene();
    let path = scene_path("job_cancel");
    bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap();
    let before = scene.lightmaps.clone();

    // Heavy enough that the cancel lands long before the bake could finish.
    let heavy = BakeSettings {
        samples: 4096,
        ..quick()
    };
    let mut job = LightmapBakeJob::start(&mut scene, Some(&path), heavy).unwrap();
    job.cancel();
    assert!(job.is_cancelled());
    assert_eq!(finish(&mut job, &mut scene), BakeOutcome::Cancelled);
    assert_eq!(scene.lightmaps, before);
    assert!(
        Path::new(&before.pages[0]).exists(),
        "its pages stay on disk"
    );
}

#[test]
fn an_unsaved_scene_cannot_start_a_background_bake() {
    let (mut scene, _, _) = scene();
    let err = LightmapBakeJob::start(&mut scene, None, quick()).err();
    assert_eq!(err.as_deref(), Some(UNSAVED));
}
