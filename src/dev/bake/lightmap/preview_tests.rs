//! The editor's progressive bake on the live scene (#1970): each pass's preview is
//! shown through preview pages, a save meanwhile writes the scene's own lightmaps,
//! a cancel puts them back, and a finished bake leaves no preview page behind.

use std::time::Duration;

use super::tests::{quick, scene, scene_path};
use super::*;
use crate::scene::lighting::lightmap::Pass;

/// The `lightmap_preview_*` pages beside `path`.
pub(super) fn preview_pages(path: &str) -> usize {
    let Ok(entries) = std::fs::read_dir(lightmap_dir(path)) else {
        return 0;
    };
    let names = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned());
    names.filter(|n| n.starts_with("lightmap_preview_")).count()
}

/// Poll `job` (showing each pass, as the editor does each frame) until it ends,
/// with how many passes it showed.
pub(super) fn run(job: &mut LightmapBakeJob, scene: &mut Scene) -> (BakeOutcome, usize) {
    let mut shown = 0;
    for _ in 0..12_000 {
        shown += usize::from(job.show_preview(scene).is_some());
        if let Some(outcome) = job.poll(scene) {
            return (outcome, shown);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the bake never finished");
}

/// A bake of many one-path passes started on `scene`, still going once its first
/// pass is shown, and that pass.
pub(super) fn showing(scene: &mut Scene, path: &str) -> (LightmapBakeJob, Pass) {
    let slow = BakeSettings {
        samples: 4096,
        samples_per_pass: 1,
        ..quick()
    };
    let mut job = LightmapBakeJob::start(scene, Some(path), slow, None).unwrap();
    for _ in 0..12_000 {
        if let Some(pass) = job.show_preview(scene) {
            return (job, pass);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("no pass was shown");
}

#[test]
fn a_preview_is_shown_but_never_saved_and_a_cancel_puts_the_lightmaps_back() {
    let (mut scene, floor, _) = scene();
    let path = scene_path("preview_cancel");
    bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap();
    let before = (*scene.lightmaps).clone();
    let (mut job, pass) = showing(&mut scene, &path);
    assert!(pass.done >= 1 && pass.samples == pass.done && pass.target == 4096);
    assert!(
        preview_pages(&path) > 0,
        "the pass is written beside the scene"
    );
    assert_ne!(scene.lightmaps.pages, before.pages, "and shown");
    assert!(
        scene.lightmaps.pages[0].ends_with(".png"),
        "uncompressed, one level"
    );
    assert!(scene.lightmaps.get(floor).is_some());
    assert_eq!(*scene.lightmaps.saved(), before);
    let saved = crate::scene::to_scene_data(&scene).lightmaps;
    assert_eq!(saved, before, "a save writes the scene's own lightmaps");
    job.cancel();
    assert_eq!(run(&mut job, &mut scene).0, BakeOutcome::Cancelled);
    assert_eq!(*scene.lightmaps, before);
    assert_eq!(preview_pages(&path), 0, "the preview pages are removed");
    assert!(std::path::Path::new(&before.pages[0]).exists());
}

#[test]
fn a_finished_progressive_bake_writes_the_one_shot_pages() {
    let (mut sync, _, _) = scene();
    let settings = BakeSettings {
        samples: 6,
        samples_per_pass: 2,
        ..quick()
    };
    bake_scene_lightmaps(&mut sync, Some(&scene_path("preview_sync")), &settings).unwrap();
    let one_shot = std::fs::read(&sync.lightmaps.pages[0]).unwrap();
    let (mut scene, _, _) = scene();
    let path = scene_path("preview_done");
    let mut job = LightmapBakeJob::start(&mut scene, Some(&path), settings, None).unwrap();
    let (outcome, shown) = run(&mut job, &mut scene);
    assert_eq!(outcome, BakeOutcome::Baked(1));
    assert!(shown <= 2, "two passes before the last, at most: {shown}");
    assert_eq!(std::fs::read(&scene.lightmaps.pages[0]).unwrap(), one_shot);
    assert!(scene.lightmaps.shown_over.is_none());
    assert_eq!(
        preview_pages(&path),
        0,
        "the finished write cleared the previews"
    );
}

#[test]
fn a_pass_line_says_the_pass_its_samples_and_the_time_left() {
    let pass = Pass {
        done: 3,
        passes: 8,
        samples: 24,
        target: 64,
    };
    let line = job::pass_line(pass, Some(Duration::from_secs(185)));
    assert_eq!(line, "pass 3/8 (24/64 samples), ~3 min 5 s left");
    assert_eq!(job::pass_line(pass, None), "pass 3/8 (24/64 samples)");
}
