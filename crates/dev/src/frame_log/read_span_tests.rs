use super::{segment, ts};
use crate::frame_log::read::*;

#[test]
fn a_full_read_stops_and_counts_only_its_own_span() {
    let dir = rusty_test_support::temp::dir();
    let f = |t: u64| format!(r#"{{"ch":"frames","t":{t},"tick":0,"frame":{t},"ms":16}}"#);
    let old: Vec<String> = (1..=50).map(f).collect();
    let mut old: Vec<&str> = old.iter().map(String::as_str).collect();
    old.insert(10, r#"{"ch":"dropped","t":10,"tick":0,"records":2000}"#);
    old.push("a cut line from long ago");
    segment(&dir, 0, &old);
    let new: Vec<String> = (100..=120).map(f).collect();
    segment(&dir, 1, &new.iter().map(String::as_str).collect::<Vec<_>>());
    let q = LogQuery {
        limit: Some(10),
        ..LogQuery::default()
    };
    let got = read(&dir, &q, 200);
    assert_eq!(ts(&got)[0], 111);
    assert_eq!(
        (got.truncated, got.dropped, got.skipped),
        (11, 0, 0),
        "segment 0 unread"
    );
    let back = LogQuery {
        last_seconds: Some(0.1),
        ..LogQuery::default()
    };
    let got = read(&dir, &back, 200);
    assert_eq!((got.records.len(), got.dropped, got.skipped), (21, 0, 0));
    let all = LogQuery {
        limit: Some(usize::MAX),
        ..LogQuery::default()
    };
    let got = read(&dir, &all, 200);
    assert_eq!(
        (got.dropped, got.skipped),
        (2000, 1),
        "the whole log is its span"
    );
}
