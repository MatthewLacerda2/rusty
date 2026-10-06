//! [`run`]'s persistence (#889): the report is kept in the out dir, and the next
//! run reads it back and prints its change against it. Renders when an adapter is
//! there (hence `gpu_`), but every assertion holds without one: `frame_ms` and the
//! system rows are the sim's.

use super::*;

/// The `frame_ms` row of a printed report, split into its columns.
fn frame_row(text: &str) -> Vec<String> {
    let line = text.lines().find(|l| l.starts_with("frame_ms")).unwrap();
    line.split_whitespace().map(str::to_string).collect()
}

#[test]
fn gpu_run_keeps_its_report_and_the_next_run_compares_against_it() {
    let out = crate::test_temp::dir().join(format!("bench_run_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let mut h = Harness::new(out.clone(), "");
    assert_eq!(read(&out), None, "a fresh out dir has no last run");

    let first = run(&mut h, 1);
    assert_eq!(first.frames, 1);
    let kept = read(&out).expect("run keeps bench.json in the out dir");
    assert_eq!(kept.frames, first.frames);
    let names = |r: &Report| r.metrics.iter().map(|m| m.0.clone()).collect::<Vec<_>>();
    assert_eq!(names(&kept), names(&first));
    assert!(names(&first).iter().any(|m| m == "frame_ms"));
    let text = std::fs::read_to_string(out.join("bench.txt")).unwrap();
    assert!(text.contains("(no previous report"), "{text}");
    assert_eq!(frame_row(&text).len(), 3, "nothing to compare against yet");

    let second = run(&mut h, 1);
    assert_eq!(second.frames, 1);
    let text = std::fs::read_to_string(out.join("bench.txt")).unwrap();
    assert!(!text.contains("(no previous report"), "{text}");
    assert!(
        frame_row(&text).len() > 3,
        "frame_ms carries its change against the first run: {text}"
    );
}
