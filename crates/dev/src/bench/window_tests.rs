use std::cell::RefCell;
use std::rc::Rc;

use super::super::report::{read, Report, Summary};
use super::*;

fn row(name: &str, avg: f64) -> (String, Summary) {
    (name.to_string(), Summary::new(avg, avg))
}

#[test]
fn a_window_is_its_rows_prefixed_and_reads_back_by_their_plain_names() {
    let window = Report {
        frames: 2,
        metrics: vec![row("cpu_ms", 1.0), row("cpu.follow_bones", 0.25)],
        warnings: vec!["the CPU clock moved".to_string()],
        ..Report::default()
    };
    let kept = prefixed(window.clone(), "firefight");
    let names: Vec<_> = kept.metrics.iter().map(|m| m.0.as_str()).collect();
    assert_eq!(names, ["firefight.cpu_ms", "firefight.cpu.follow_bones"]);
    assert_eq!(kept.warnings, ["window `firefight`: the CPU clock moved"]);
    let mut run = Report {
        metrics: vec![row("cpu_ms", 9.0)],
        ..Report::default()
    };
    run.metrics.extend(kept.metrics);
    assert_eq!(window_of(&run, "firefight").metrics, window.metrics);
    assert!(window_of(&run, "other").metrics.is_empty());
}

#[test]
fn a_window_compares_against_its_rows_and_reads_absent_without_them() {
    let now = Report {
        frames: 1,
        metrics: vec![row("cpu_ms", 2.0)],
        ..Report::default()
    };
    let with = Report {
        metrics: vec![row("cpu_ms", 9.0), row("firefight.cpu_ms", 1.0)],
        ..Report::default()
    };
    let table = now.render(Some(&window_of(&with, "firefight")));
    assert!(table.contains("+1.000 (+100.0%)"), "{table}");
    let without = Report {
        metrics: vec![row("cpu_ms", 9.0)],
        ..Report::default()
    };
    let table = now.render(Some(&window_of(&without, "firefight")));
    let line = table.lines().find(|l| l.starts_with("cpu_ms")).unwrap();
    assert!(line.ends_with("absent on the last run"), "{table}");
}

#[test]
fn a_window_name_is_a_lowercase_word_measured_once() {
    let out = rusty_test_support::temp::dir().join(format!("bench_win_{}", std::process::id()));
    let mut h = Harness::new(out, "");
    let mut run = Run::default();
    for bad in ["", "Fire", "fire.fight"] {
        let err = run.then(&mut h, 1, bad).unwrap_err();
        assert!(err.contains("lowercase word"), "{err}");
    }
    assert_eq!(h.frame(), 0, "a refused window measures nothing");
}

#[test]
fn gpu_a_run_keeps_both_windows_and_the_next_compares_each() {
    let out = rusty_test_support::temp::dir().join(format!("bench_wins_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let h = Rc::new(RefCell::new(Harness::new(out.clone(), "")));
    let lua = mlua::Lua::new();
    let t = lua.create_table().unwrap();
    super::super::register(&lua, &h, &t).unwrap();
    lua.globals().set("Harness", t).unwrap();
    let both = "local q = Harness.Bench(1); local f = Harness.Bench(1, 'firefight'); \
                return q.metrics.frame_ms ~= nil and f.metrics['firefight.frame_ms'] ~= nil";
    assert!(lua.load(both).eval::<bool>().unwrap());
    let kept = read(&out).expect("the run is kept");
    let has = |m: &str| kept.has(m);
    assert!(has("frame_ms") && has("firefight.frame_ms") && has("firefight.cpu_ms"));
    assert!(!has("firefight.firefight.frame_ms"));
    let text = std::fs::read_to_string(out.join("bench.txt")).unwrap();
    assert!(text.contains("window `firefight`"), "{text}");
    let err = lua
        .load("Harness.Bench(1, 'firefight')")
        .exec()
        .unwrap_err();
    assert!(err.to_string().contains("already measured"), "{err}");

    assert!(lua.load(both).eval::<bool>().unwrap());
    let text = std::fs::read_to_string(out.join("bench.txt")).unwrap();
    let frame_rows: Vec<_> = text.lines().filter(|l| l.starts_with("frame_ms")).collect();
    assert_eq!(frame_rows.len(), 2, "one table a window: {text}");
    for line in frame_rows {
        assert!(
            line.split_whitespace().count() > 3,
            "a change column: {line}"
        );
        assert!(!line.contains("absent"), "{line}");
    }
}
