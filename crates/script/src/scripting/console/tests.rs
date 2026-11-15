use super::*;

#[test]
fn a_repeated_record_counts_up_on_the_newest_entry() {
    let mut log = ConsoleLogs::new();
    log.repeated("[Physics] skipped".into(), LogLevel::Warning, 1);
    log.repeated("[Physics] skipped".into(), LogLevel::Warning, 2);
    assert_eq!(log.messages.len(), 1);
    assert_eq!(log.messages[0].0, "[Physics] skipped (×3)");
}

#[test]
fn another_entry_between_starts_a_new_run() {
    let mut log = ConsoleLogs::new();
    log.repeated("[Asset] gone".into(), LogLevel::Warning, 1);
    log.info("script output".into());
    log.repeated("[Asset] gone".into(), LogLevel::Warning, 1);
    log.repeated("[Asset] gone".into(), LogLevel::Error, 1);
    let texts: Vec<_> = log.messages.iter().map(|(m, _)| m.as_str()).collect();
    assert_eq!(
        texts,
        [
            "[Asset] gone",
            "script output",
            "[Asset] gone",
            "[Asset] gone"
        ]
    );
}

#[test]
fn a_demo_is_fed_marks_and_every_repeat_only_while_it_records() {
    let mut log = ConsoleLogs::new();
    let mark = |label: &str| Mark {
        label: label.into(),
        value: None,
    };
    log.mark(mark("dropped"));
    log.feed_demo(true);
    log.mark(mark("kept"));
    log.info("once".into());
    log.repeated("again".into(), LogLevel::Warning, 1);
    log.repeated("again".into(), LogLevel::Warning, 2);
    let feed = log.take_feed();
    assert_eq!(feed.marks, [mark("kept")]);
    let lines: Vec<_> = feed.lines.iter().map(|(m, _)| m.as_str()).collect();
    assert_eq!(
        lines,
        ["once", "again", "again (×2)"],
        "a folded repeat too"
    );
    assert_eq!(log.take_feed(), DemoFeed::default(), "taken once");
    log.feed_demo(false);
    log.mark(mark("dropped"));
    assert_eq!(log.take_feed(), DemoFeed::default());
}

#[test]
fn since_reads_what_came_after_the_mark_even_past_the_cap() {
    let mut log = ConsoleLogs::new();
    log.info("before".into());
    let mark = log.logged();
    assert_eq!(log.since(mark).count(), 0);
    for i in 0..150 {
        log.error(format!("e{i}"));
    }
    let after: Vec<_> = log.since(mark).map(|(m, _)| m.as_str()).collect();
    assert_eq!(after.len(), 100, "only what the buffer still holds");
    assert_eq!((after[0], after[99]), ("e50", "e149"));
}

#[test]
fn kept_warnings_outlive_the_cap_and_fold_repeats() {
    let mut log = ConsoleLogs::new();
    log.warn("before keeping".into());
    assert!(log.warnings().is_empty(), "kept only once asked");
    log.keep_warnings();
    log.error("broken".into());
    log.repeated("[Asset] gone".into(), LogLevel::Warning, 1);
    log.repeated("[Asset] gone".into(), LogLevel::Warning, 2);
    for i in 0..150 {
        log.info(format!("i{i}"));
    }
    let kept: Vec<_> = log
        .warnings()
        .iter()
        .map(|(m, l)| (m.as_str(), l))
        .collect();
    assert_eq!(
        kept,
        [
            ("broken", &LogLevel::Error),
            ("[Asset] gone (×3)", &LogLevel::Warning)
        ]
    );
}

#[test]
fn the_file_feed_keeps_every_line_past_the_cap_and_counts_repeats() {
    let mut log = ConsoleLogs::new();
    log.info("before".into());
    log.feed_file(true);
    for i in 0..150 {
        log.info(format!("l{i}"));
    }
    log.repeated("[Physics] skipped".into(), LogLevel::Warning, 1);
    log.repeated("[Physics] skipped".into(), LogLevel::Warning, 2);
    let feed = log.take_file_feed();
    assert_eq!(feed.lines.len(), 151, "past the buffer's 100, none before");
    let last = feed.lines.last().unwrap();
    assert_eq!((last.msg.as_str(), last.times), ("[Physics] skipped", 3));
    // The buffer folded the run long ago; the next take still sees each repeat.
    log.repeated("[Physics] skipped".into(), LogLevel::Warning, 1);
    let again = log.take_file_feed();
    assert_eq!(
        again.lines.len(),
        1,
        "a repeat in a later frame is written again"
    );
    for i in 0..FILE_FEED_CAP + 5 {
        log.info(format!("flood {i}"));
    }
    assert_eq!(log.take_file_feed().lost, 5);
    log.feed_file(false);
    log.info("unfed".into());
    assert_eq!(log.take_file_feed(), FileFeed::default());
}
