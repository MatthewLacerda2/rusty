//! The frame log read back (#2206): a cut or bad line, the look-back and the
//! limit, the filters, and a frame's GPU times joined onto it.

use std::io::Write;

use super::*;
use crate::frame_log::segments::segment_path;

/// Write `lines` as segment `n` of `dir`.
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

/// A look-back is returned whole; a limit that cuts says how many it left out;
/// one bad byte costs its line, not its segment; the drops in the span are summed.
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

/// GPU times ride on a later frame's line under `gpu_frame`; the read puts them
/// on the frame they measure, so a hitch names its own heaviest pass. A drawn
/// frame whose times are not in the log says so, and an earlier session's frame
/// of the same index never takes a later session's times.
#[test]
fn gpu_times_are_joined_onto_the_frame_they_measure() {
    let dir = rusty_test_support::temp::dir();
    let f = |t: u64, frame: u64, ms: u64, gpu: &str| {
        let head = format!(r#"{{"ch":"frames","t":{t},"tick":0,"frame":{frame},"ms":{ms}"#);
        format!("{head}{gpu}}}")
    };
    // As `record::frame` writes them: `gpu_frame` and `cpu_ms` in the head.
    let gpu = |of: u64, pass: &str| {
        format!(r#","gpu_frame":{of},"cpu_ms":1,"gpu_ms":9,"gpu_passes":{{"{pass}":9}}"#)
    };
    let drew = r#","cpu_ms":1"#;
    let lines = [
        f(1, 1, 40, drew),               // an earlier session's frame 1
        f(2, 1, 40, drew),               // this session: frame 1 hitches
        f(3, 2, 16, &gpu(1, "shadows")), // its times, a frame late
        f(4, 3, 16, &gpu(2, "forward")),
    ];
    segment(
        &dir,
        0,
        &lines.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    let q = LogQuery {
        over_ms: Some(33.0),
        ..LogQuery::default()
    };
    let got = read(&dir, &q, 10);
    assert_eq!(ts(&got), [1, 2]);
    let (old, hitch) = (&got.records[0], &got.records[1]);
    assert_eq!(hitch["heaviest_pass"], "shadows", "{hitch}");
    assert_eq!(hitch["gpu_passes"], serde_json::json!({ "shadows": 9 }));
    assert!(hitch.get("gpu_frame").is_none(), "{hitch}");
    assert_eq!(old["gpu_resolved"], false, "{old}");
    let all = read(&dir, &LogQuery::default(), 10);
    let newest = &all.records[3];
    assert_eq!(newest["gpu_resolved"], false, "not resolved yet: {newest}");
    assert_eq!(
        all.records[2]["gpu_passes"],
        serde_json::json!({ "forward": 9 })
    );
}
