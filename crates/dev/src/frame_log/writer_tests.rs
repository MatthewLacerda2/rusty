use super::*;
use crate::frame_log::segments::{numbered, segment_path, Segments};

fn line(i: usize) -> String {
    format!(r#"{{"ch":"frames","t":{},"i":{i:04}}}"#, now_ms())
}

fn files(dir: &Path) -> Vec<(u64, u64)> {
    let sizes = numbered(dir).into_iter();
    sizes
        .map(|(n, p)| (n, std::fs::metadata(p).unwrap().len()))
        .collect()
}

#[test]
fn segments_rotate_and_hold_the_budget() {
    let dir = rusty_test_support::temp::dir();
    let mut segments = Segments::open(&dir, 1000, None).unwrap();
    for i in 0..200 {
        segments.write(&line(i));
    }
    segments.flush();
    let kept = files(&dir);
    let total: u64 = kept.iter().map(|(_, bytes)| bytes).sum();
    assert!(total <= 1000, "{total} bytes in {kept:?}");
    assert!(kept.iter().all(|(_, bytes)| *bytes <= 100), "{kept:?}");
    assert!(kept[0].0 > 0, "the oldest segments were deleted: {kept:?}");
    let newest = std::fs::read_to_string(numbered(&dir).last().unwrap().1.clone()).unwrap();
    assert!(newest.trim_end().ends_with(r#""i":0199}"#), "{newest}");
}

#[test]
fn a_restart_continues_after_the_newest_segment() {
    let dir = rusty_test_support::temp::dir();
    let mut first = Segments::open(&dir, 10_000, None).unwrap();
    first.write(&line(0));
    first.flush();
    drop(first);
    let mut second = Segments::open(&dir, 10_000, None).unwrap();
    second.write(&line(1));
    second.flush();
    drop(second);
    let numbers: Vec<u64> = files(&dir).into_iter().map(|(n, _)| n).collect();
    assert_eq!(numbers, [0, 1], "the earlier session's segment is kept");
    assert_eq!(
        Segments::open(&dir, 0, None).unwrap().closed.len(),
        0,
        "a budget of 0 keeps none"
    );
}

#[test]
fn the_period_drops_a_stale_segment() {
    let dir = rusty_test_support::temp::dir();
    let mut segments = Segments::open(&dir, 1_000_000, Some(60_000)).unwrap();
    segments.write(&line(0));
    segments.close();
    segments.write(&line(1));
    segments.prune(now_ms());
    assert_eq!(segments.closed.len(), 1, "a fresh segment stays");
    segments.closed[0].newest = 0;
    segments.prune(now_ms());
    assert!(segments.closed.is_empty());
    assert_eq!(files(&dir).len(), 1, "only the open one is left");
}

#[test]
fn a_writer_behind_drops_and_counts() {
    let dir = rusty_test_support::temp::dir();
    let (tx, rx) = sync_channel(QUEUE);
    let dropped = Arc::new(Dropped::default());
    let mut writer = Writer {
        tx: Some(tx),
        dropped: Arc::clone(&dropped),
        thread: None,
    };
    for i in 0..QUEUE + 2 {
        writer.send(vec![line(i), line(i)], i as u64);
    }
    let records = dropped.records.load(Ordering::Relaxed);
    assert_eq!(records, 4, "two frames of two lines");
    drop(writer);
    drain(rx, Segments::open(&dir, 1 << 20, None).unwrap(), &dropped);
    let text: String = numbered(&dir)
        .into_iter()
        .map(|(_, p)| std::fs::read_to_string(p).unwrap())
        .collect();
    let first = text.lines().next().unwrap();
    assert!(
        first.starts_with(r#"{"ch":"dropped","t":"#)
            && first.contains(&format!(r#","tick":{},"#, QUEUE + 1))
            && first.contains(r#""records":4"#),
        "{first}"
    );
    assert_eq!(text.lines().count(), 1 + 2 * QUEUE);
}

#[test]
fn a_dropped_writer_has_written_everything() {
    let dir = rusty_test_support::temp::dir();
    let mut writer = Writer::start(&dir, 1 << 20, None).unwrap();
    for i in 0..50 {
        writer.send(vec![line(i)], 0);
    }
    writer.send(Vec::new(), 0);
    drop(writer);
    let text = std::fs::read_to_string(segment_path(&dir, 0)).unwrap();
    assert_eq!(text.lines().count(), 50);
}

#[test]
fn a_second_writer_on_one_folder_is_refused() {
    let dir = rusty_test_support::temp::dir();
    let first = Writer::start(&dir, 1 << 20, None).unwrap();
    let Err(err) = Writer::start(&dir, 1 << 20, None) else {
        panic!("a second writer started");
    };
    assert!(err.to_string().contains("another rusty process"), "{err}");
    drop(first);
    assert!(Writer::start(&dir, 1 << 20, None).is_ok());
}
