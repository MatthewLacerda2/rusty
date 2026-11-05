//! The occlusion bake's runs (#1714): the editor's worker and a script's logged
//! run write the same bytes as the in-line bake, and the log says its step.

use std::cell::RefCell;
use std::time::Duration;

use super::occlusion::{bake_occlusion_logged, OcclusionBakeJob};
use crate::dev::bake::progress::{BakeLog, Line};
use crate::scene::lighting::occlusion::test_level::corridors;
use crate::scene::lighting::occlusion::{
    bake_occlusion, encode, gather_occlusion, OcclusionProgress,
};

#[test]
fn the_worker_and_the_logged_run_bake_what_the_inline_bake_does() {
    let scene = corridors();
    let inline = encode(&bake_occlusion(&scene, 2).0.expect("occluders"));
    let mut job = OcclusionBakeJob::start(&scene).unwrap();
    let baked = loop {
        if let Some(done) = job.poll() {
            break done.unwrap();
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let (done, total) = job.cells();
    assert_eq!((done, total), (baked.report.cells, baked.report.cells));
    assert_eq!(encode(baked.data.as_ref().unwrap()), inline);

    let lines: RefCell<Vec<Line>> = RefCell::default();
    let mut log = BakeLog::new(|| Duration::ZERO, |l| lines.borrow_mut().push(l));
    let logged = bake_occlusion_logged(&scene, &mut log).unwrap();
    drop(log);
    assert_eq!(encode(logged.data.as_ref().unwrap()), inline);
    // A frozen clock: the step's first line, and no other step's.
    let lines: Vec<String> = lines.into_inner().into_iter().map(|l| l.text).collect();
    assert_eq!(lines[0], "[Lighting] occlusion preparing", "{lines:?}");
    assert!(lines.iter().all(|l| l.starts_with("[Lighting] occlusion")));
}

#[test]
fn a_cancelled_bake_returns_nothing() {
    let progress = OcclusionProgress::default();
    progress.cancel();
    let input = gather_occlusion(&corridors());
    assert_eq!(
        input.bake(2, &progress),
        None,
        "what a dropped job's worker sees"
    );
}

#[test]
fn the_summary_counts_the_never_culled_and_says_a_level_too_large() {
    use crate::dev::bake::generate_lighting::GenerateReport;
    use crate::scene::lighting::occlusion::OcclusionBakeReport;
    let occlusion = OcclusionBakeReport {
        objects: 9,
        never_culled: 2,
        cells: 40,
        cell_size: 2.0,
        bytes: 900,
        ..Default::default()
    };
    let mut report = GenerateReport {
        lightmaps: 0,
        probes: Default::default(),
        occlusion,
        occlusion_ms: 5,
        saved: crate::scene::LightingSave::Written,
    };
    let line = report.summary();
    assert!(
        line.contains("40 cell(s) of 2.0 m, 9 object(s), 2 never culled"),
        "{line}"
    );
    report.occlusion = OcclusionBakeReport {
        too_large: true,
        ..Default::default()
    };
    let line = report.summary();
    assert!(
        line.contains("no occlusion culling: the level needs cells over 4 m"),
        "{line}"
    );
}
