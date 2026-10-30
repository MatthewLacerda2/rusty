/// The console log buffer and severity levels shared by gameplay scripts, the
/// REPL and the editor's bottom panel. The `print` hook and every `Debug.*`
/// binding funnel through here.
pub struct ConsoleLogs {
    pub messages: Vec<(String, LogLevel)>,
    /// The record the newest entry collapses, and how many times it has arrived in a
    /// row ([`ConsoleLogs::repeated`]).
    repeat: Option<(String, u32)>,
    /// How many entries were ever added, the dropped ones included: a mark for
    /// [`ConsoleLogs::since`].
    logged: u64,
    /// What a recording demo has not taken yet (#1597); `None` while no demo
    /// records, which makes a mark a no-op.
    feed: Option<DemoFeed>,
}

/// What the console hands a recording demo's index each tick (#1597).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DemoFeed {
    pub marks: Vec<Mark>,
    /// Every line as it came, a repeat the buffer folds into `msg (×N)` included.
    pub lines: Vec<(String, LogLevel)>,
}

/// A game fact a script marked for a demo's index (`Debug.Mark(label [, value])`).
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub label: String,
    pub value: Option<super::Plain>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

impl Default for ConsoleLogs {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsoleLogs {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            repeat: None,
            logged: 0,
            feed: None,
        }
    }

    /// Feed a demo from now on (`true`, a demo started recording), or stop
    /// (`false`): marks are dropped as they come.
    pub fn feed_demo(&mut self, feed: bool) {
        self.feed = feed.then(DemoFeed::default);
    }

    /// Note a mark; a no-op unless a demo records.
    pub fn mark(&mut self, mark: Mark) {
        if let Some(feed) = &mut self.feed {
            feed.marks.push(mark);
        }
    }

    /// The marks and lines since the last take, oldest first.
    pub fn take_feed(&mut self) -> DemoFeed {
        self.feed.as_mut().map(std::mem::take).unwrap_or_default()
    }

    fn feed_line(&mut self, msg: &str, level: &LogLevel) {
        if let Some(feed) = &mut self.feed {
            feed.lines.push((msg.to_string(), level.clone()));
        }
    }

    pub fn info(&mut self, msg: String) {
        self.add_log(msg, LogLevel::Info);
    }

    pub fn warn(&mut self, msg: String) {
        self.add_log(msg, LogLevel::Warning);
    }

    pub fn error(&mut self, msg: String) {
        self.add_log(msg, LogLevel::Error);
    }

    /// Log a record that arrived `times` times in a row. When it repeats the newest
    /// entry, that entry counts up instead (`msg (×3)`), as Unity's Console collapses
    /// a repeated warning, so one warning raised every frame cannot push everything
    /// else out of the buffer.
    pub fn repeated(&mut self, msg: String, level: LogLevel, times: u32) {
        self.feed_line(&Self::collapsed(&msg, times), &level);
        if let Some((text, count)) = &self.repeat {
            let newest = self.messages.last();
            if *text == msg && newest == Some(&(Self::collapsed(text, *count), level.clone())) {
                let count = count + times;
                if let Some(entry) = self.messages.last_mut() {
                    entry.0 = Self::collapsed(&msg, count);
                }
                self.repeat = Some((msg, count));
                return;
            }
        }
        self.add_entry(Self::collapsed(&msg, times), level);
        self.repeat = Some((msg, times));
    }

    /// How the console shows a record that arrived `count` times in a row.
    pub fn collapsed(msg: &str, count: u32) -> String {
        match count {
            0 | 1 => msg.to_string(),
            n => format!("{msg} (×{n})"),
        }
    }

    /// A mark to read the entries added after it with [`since`](Self::since).
    pub fn logged(&self) -> u64 {
        self.logged
    }

    /// The entries added since `mark` ([`logged`](Self::logged)) still in the buffer,
    /// oldest first.
    pub fn since(&self, mark: u64) -> impl Iterator<Item = &(String, LogLevel)> {
        let added = usize::try_from(self.logged - mark).unwrap_or(usize::MAX);
        let kept = added.min(self.messages.len());
        self.messages[self.messages.len() - kept..].iter()
    }

    fn add_log(&mut self, msg: String, level: LogLevel) {
        self.feed_line(&msg, &level);
        self.add_entry(msg, level);
    }

    fn add_entry(&mut self, msg: String, level: LogLevel) {
        self.logged += 1;
        self.repeat = None;
        if self.messages.len() >= 100 {
            self.messages.remove(0);
        }
        self.messages.push((msg, level));
    }
}

#[cfg(test)]
mod tests {
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
}
