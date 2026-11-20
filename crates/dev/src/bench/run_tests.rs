//! A run's persistence (`window::Run`, #889, #2448): the report is kept in the
//! out dir, the next run reads it back and prints its change against it, and a
//! run stopped before a later window keeps the last run's rows of that window.
//! Renders when an adapter is there (hence `gpu_`), but every assertion holds
//! without one: `frame_ms` and the system rows are the sim's.

use super::report::read;
use super::window::Run;
use super::*;

/// The `frame_ms` rows of a printed run, one a window, split into their columns.
fn frame_rows(out: &std::path::Path) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(out.join("bench.txt")).unwrap();
    let rows = text.lines().filter(|l| l.starts_with("frame_ms"));
    rows.map(|l| l.split_whitespace().map(str::to_string).collect())
        .collect()
}

fn fresh(name: &str) -> (std::path::PathBuf, Harness) {
    let out = rusty_test_support::temp::dir().join(format!("{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    (out.clone(), Harness::new(out, ""))
}

#[test]
fn gpu_run_keeps_its_report_and_the_next_run_compares_against_it() {
    let (out, mut h) = fresh("bench_run");
    assert_eq!(read(&out), None, "a fresh out dir has no last run");

    let first = Run::default().first(&mut h, 1);
    assert_eq!(first.frames, 1);
    let kept = read(&out).expect("run keeps bench.json in the out dir");
    assert_eq!(kept.frames, first.frames);
    let names = |r: &Report| r.metrics.iter().map(|m| m.0.clone()).collect::<Vec<_>>();
    assert_eq!(names(&kept), names(&first));
    assert!(names(&first).iter().any(|m| m == "frame_ms"));
    let text = std::fs::read_to_string(out.join("bench.txt")).unwrap();
    assert!(text.contains("(no previous report"), "{text}");
    assert_eq!(
        frame_rows(&out)[0].len(),
        3,
        "nothing to compare against yet"
    );

    let second = Run::default().first(&mut h, 1);
    assert_eq!(second.frames, 1);
    let text = std::fs::read_to_string(out.join("bench.txt")).unwrap();
    assert!(!text.contains("(no previous report"), "{text}");
    assert!(
        frame_rows(&out)[0].len() > 3,
        "frame_ms carries its change against the first run: {text}"
    );
}

#[test]
fn gpu_a_run_stopped_before_a_window_keeps_the_last_runs_rows_of_it() {
    let (out, mut h) = fresh("bench_stopped");
    let both = |h: &mut Harness| {
        let mut run = Run::default();
        run.first(h, 1);
        run.then(h, 1, "firefight").unwrap()
    };
    let a = both(&mut h);
    Run::default().first(&mut h, 1); // run B stops after its first window
    let kept = read(&out).unwrap();
    let a_rows = |r: &Report| {
        let rows = r.metrics.iter().filter(|m| m.0.starts_with("firefight."));
        rows.cloned().collect::<Vec<_>>()
    };
    assert_eq!(a_rows(&kept), a.metrics, "B kept A's firefight rows");
    assert_eq!(kept.windows, ["firefight"]);
    assert!(kept.has("frame_ms"), "and its own first window");

    both(&mut h); // run C: its firefight table compares against A's
    let rows = frame_rows(&out);
    assert_eq!(rows.len(), 2, "one table a window");
    for row in rows {
        assert!(
            row.len() > 3 && !row.contains(&"absent".to_string()),
            "{row:?}"
        );
    }
}
