use std::io::Write;

use super::*;
use crate::frame_log::segments::segment_path;

fn segment(dir: &Path, n: u64, lines: &[&str]) {
    let mut out = std::fs::File::create(segment_path(dir, n)).unwrap();
    for line in lines {
        writeln!(out, "{line}").unwrap();
    }
}

fn ts(read: &LogRead) -> Vec<u64> {
    read.records
        .iter()
        .map(|r| r["t"].as_u64().unwrap())
        .collect()
}

#[test]
fn a_cut_last_line_is_skipped() {
    let dir = rusty_test_support::temp::dir();
    segment(&dir, 0, &[r#"{"ch":"frames","t":1,"ms":16}"#]);
    let mut out = std::fs::File::create(segment_path(&dir, 1)).unwrap();
    let cut = concat!(r#"{"ch":"frames","t":2,"ms":16}"#, "\n", r#"{"ch":"fra"#);
    out.write_all(cut.as_bytes()).unwrap();
    let got = read(&dir, &LogQuery::default(), 10);
    assert_eq!((ts(&got), got.skipped), (vec![1, 2], 1));
}

#[test]
fn a_look_back_stops_at_the_segments_before_it() {
    let dir = rusty_test_support::temp::dir();
    segment(&dir, 0, &["not json at all"]);
    segment(
        &dir,
        1,
        &[r#"{"ch":"frames","t":1000}"#, r#"{"ch":"frames","t":4000}"#],
    );
    segment(
        &dir,
        2,
        &[r#"{"ch":"frames","t":5000}"#, r#"{"ch":"frames","t":6000}"#],
    );
    let q = LogQuery {
        last_seconds: Some(3.5),
        ..LogQuery::default()
    };
    let got = read(&dir, &q, 7000);
    assert_eq!(ts(&got), [4000, 5000, 6000]);
    assert_eq!(got.skipped, 0, "segment 0 was never read");
    let q = LogQuery {
        limit: Some(2),
        ..LogQuery::default()
    };
    assert_eq!(ts(&read(&dir, &q, 7000)), [5000, 6000], "the newest two");
}

#[test]
fn over_ms_names_the_heaviest_pass_and_system() {
    let dir = rusty_test_support::temp::dir();
    segment(
        &dir,
        0,
        &[
            r#"{"ch":"frames","t":1,"ms":16}"#,
            r#"{"ch":"frames","t":2,"ms":40,"gpu_passes":{"forward":3,"shadow":9},"systems":{"rebake_nav":27.7}}"#,
            r#"{"ch":"console","t":3,"level":"info","msg":"hi"}"#,
        ],
    );
    let q = LogQuery {
        over_ms: Some(33.0),
        ..LogQuery::default()
    };
    let got = read(&dir, &q, 10);
    assert_eq!(ts(&got), [2]);
    assert_eq!(got.records[0]["heaviest_pass"], "shadow");
    assert_eq!(got.records[0]["heaviest_system"], "rebake_nav");
    let console = LogQuery {
        channel: Some("console".into()),
        ..LogQuery::default()
    };
    assert_eq!(ts(&read(&dir, &console, 10)), [3]);
    let passes = LogQuery {
        channel: Some("gpu_passes".into()),
        ..LogQuery::default()
    };
    assert_eq!(
        ts(&read(&dir, &passes, 10)),
        [2],
        "the frame that carries them"
    );
    assert!(
        read(&dir.join("none"), &q, 10).records.is_empty(),
        "no log is no records"
    );
}

#[test]
fn a_look_back_is_whole_and_a_cut_is_counted() {
    let dir = rusty_test_support::temp::dir();
    let frames: Vec<String> = (1..=700)
        .map(|t| format!(r#"{{"ch":"frames","t":{t},"tick":{t},"frame":{t},"ms":16}}"#))
        .collect();
    let mut lines: Vec<&str> = frames.iter().map(String::as_str).collect();
    lines.push(r#"{"ch":"dropped","t":700,"tick":700,"records":3}"#);
    segment(&dir, 0, &lines);
    let back = LogQuery {
        last_seconds: Some(1.0),
        ..LogQuery::default()
    };
    let got = read(&dir, &back, 1000);
    assert_eq!((got.records.len(), got.truncated, got.dropped), (701, 0, 3));
    let got = read(&dir, &LogQuery::default(), 1000);
    assert_eq!((got.records.len(), got.truncated), (DEFAULT_LIMIT, 101));
    let mut out = std::fs::File::create(segment_path(&dir, 1)).unwrap();
    // One bad byte costs its line, not its segment: t 802 is still read.
    out.write_all(b"{\"ch\":\"console\",\"t\":801,\"msg\":\"\xff\"}\n")
        .unwrap();
    out.write_all(b"{\"ch\":\"console\",\"t\":802,\"msg\":\"ok\"}\n")
        .unwrap();
    let got = read(
        &dir,
        &LogQuery {
            limit: Some(2),
            ..back
        },
        1000,
    );
    assert_eq!((ts(&got), got.skipped), (vec![700, 802], 1));
}

#[path = "read_span_tests.rs"]
mod span;
