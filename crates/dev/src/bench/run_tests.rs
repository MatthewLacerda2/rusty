use super::report::read;
use super::window::Run;
use super::*;

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
    Run::default().first(&mut h, 1);
    let kept = read(&out).unwrap();
    let a_rows = |r: &Report| {
        let rows = r.metrics.iter().filter(|m| m.0.starts_with("firefight."));
        rows.cloned().collect::<Vec<_>>()
    };
    assert_eq!(a_rows(&kept), a.metrics, "B kept A's firefight rows");
    assert_eq!(kept.windows, ["firefight"]);
    assert!(kept.has("frame_ms"), "and its own first window");

    both(&mut h);
    let rows = frame_rows(&out);
    assert_eq!(rows.len(), 2, "one table a window");
    for row in rows {
        assert!(
            row.len() > 3 && !row.contains(&"absent".to_string()),
            "{row:?}"
        );
    }
}
