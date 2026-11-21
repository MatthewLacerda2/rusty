use std::cell::RefCell;
use std::time::Duration;

use std::sync::Arc;

use super::{bake_occlusion_logged, CellClocks, OcclusionBakeJob};
use crate::bake::progress::{BakeLog, Line};
use rusty_lighting::occlusion::test_level::corridors;
use rusty_lighting::occlusion::{bake_occlusion, gather_occlusion, OcclusionProgress, Reach};
use rusty_scene::lighting::occlusion::encode;

#[test]
fn the_worker_and_the_logged_run_bake_what_the_inline_bake_does() {
    let scene = corridors();
    let inline = encode(&bake_occlusion(&scene, 2).0.expect("occluders"));
    let mut job = OcclusionBakeJob::start(&scene, Reach::everywhere()).unwrap();
    let baked = loop {
        if let Some(done) = job.poll() {
            break done.unwrap();
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let (done, total) = job.cells();
    assert_eq!((done, total), (baked.report.cells, baked.report.cells));
    assert_eq!(encode(baked.data.as_ref().unwrap()), inline);
    assert_eq!(baked.cells.len(), baked.report.cells, "every cell timed");

    let lines: RefCell<Vec<Line>> = RefCell::default();
    let mut log = BakeLog::new(|| Duration::ZERO, |l| lines.borrow_mut().push(l));
    let logged = bake_occlusion_logged(&scene, Reach::everywhere(), &mut log).unwrap();
    drop(log);
    assert_eq!(encode(logged.data.as_ref().unwrap()), inline);
    assert_eq!(logged.cells.len(), logged.report.cells);
    let lines: Vec<String> = lines.into_inner().into_iter().map(|l| l.text).collect();
    assert_eq!(lines[0], "[Lighting] 0:00 occlusion preparing", "{lines:?}");
    assert!(lines.iter().all(|l| l.contains(" occlusion ")));
}

#[test]
fn an_observed_bake_writes_the_bytes_an_unobserved_one_does() {
    let input = gather_occlusion(&corridors());
    let plain = input.bake(3, &OcclusionProgress::default()).unwrap();
    let clocks = Arc::new(CellClocks::default());
    let observed = input.bake(3, &OcclusionProgress::observed(clocks.clone()));
    let observed = observed.unwrap();
    assert_eq!(observed, plain, "the observer never reaches a cell");
    let times = clocks.times();
    assert_eq!(times.len(), plain.1.cells);
    assert!(
        times.iter().flatten().sum::<Duration>() > Duration::ZERO,
        "the cells ran"
    );
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
    use crate::bake::generate_lighting::GenerateReport;
    use rusty_lighting::occlusion::OcclusionBakeReport;
    let occlusion = OcclusionBakeReport {
        objects: 9,
        never_culled: 2,
        cells: 40,
        baked_cells: 40,
        cell_size: 2.0,
        bytes: 900,
        ..Default::default()
    };
    let mut report = GenerateReport {
        lightmaps: 0,
        probes: Default::default(),
        occlusion,
        occlusion_ms: 5,
        occlusion_kept: false,
        saved: rusty_edit::LightingSave::Written,
    };
    let line = report.summary();
    assert!(
        line.contains("40 cell(s) of 2.0 m (no fresh navmesh: every cell sampled; "),
        "{line}"
    );
    assert!(line.contains("), 9 object(s), 2 never culled"), "{line}");
    report.occlusion.navmesh_missed = true;
    let line = report.summary();
    assert!(line.contains(super::summary::MISSED), "{line}");
    report.occlusion.baked_cells = 12;
    report.occlusion.floor = rusty_scene::lighting::occlusion::FloorSource::Navmesh;
    report.occlusion.floor_coverage = Some(0.25);
    let line = report.summary();
    let sampled = "(12 sampled, where a camera can be over a navmesh covering 25% of the level)";
    assert!(
        line.contains(&format!("40 cell(s) of 2.0 m {sampled}, 9")),
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

#[test]
fn the_editors_generate_keeps_a_current_pvs_on_its_first_poll() {
    let mut scene = corridors();
    let (data, report) = bake_occlusion(&scene, 2);
    scene.put_baked_occlusion(data);
    let mut job = OcclusionBakeJob::generate(&scene, Reach::everywhere()).unwrap();
    let kept = job.poll().expect("finished at once").unwrap();
    assert!(kept.kept && kept.data.is_none());
    assert_eq!(kept.report, report, "the held PVS's counts");
    assert!(job.poll().is_none(), "the job is spent");
}
